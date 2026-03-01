<?php

namespace App\Http\Controllers;

use App\Models\Payment;
use App\Models\Order;
use App\Models\User;
use App\Events\PaymentCompleted;
use App\Events\RefundProcessed;
use Illuminate\Http\Request;
use Illuminate\Http\JsonResponse;
use Illuminate\Support\Facades\Log;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;
use GuzzleHttp\Client as GuzzleClient;

class PaymentController extends Controller
{
    private GuzzleClient $guzzleClient;
    private string $stripeBaseUrl;
    private string $fraudServiceUrl;

    public function __construct()
    {
        $this->stripeBaseUrl = config('services.stripe.base_url', 'https://api.stripe.com/v1');
        $this->fraudServiceUrl = config('services.fraud.base_url', 'https://fraud-detection.internal.company.com');

        $this->guzzleClient = new GuzzleClient([
            'base_uri' => $this->stripeBaseUrl,
            'timeout' => 30,
            'headers' => [
                'Authorization' => 'Bearer ' . config('services.stripe.secret_key'),
                'Content-Type' => 'application/x-www-form-urlencoded',
            ],
        ]);
    }

    /**
     * Create a new payment charge for an order.
     */
    public function charge(Request $request): JsonResponse
    {
        $validated = $request->validate([
            'order_id' => 'required|exists:orders,id',
            'payment_method_id' => 'required|string',
            'amount' => 'required|numeric|min:0.50',
            'currency' => 'sometimes|string|size:3',
            'save_method' => 'sometimes|boolean',
        ]);

        $order = Order::findOrFail($validated['order_id']);
        $user = $request->user();
        $amount = (int) ($validated['amount'] * 100); // Convert to cents
        $currency = $validated['currency'] ?? 'usd';
        $idempotencyKey = "charge_{$order->id}_" . Str::uuid();

        Log::info("Payment charge initiated", [
            'order_id' => $order->id,
            'amount' => $validated['amount'],
            'email' => $user->email,
            'ip_address' => $request->ip(),
        ]);

        // Step 1: Fraud detection check before processing payment
        $fraudScore = $this->checkFraudRisk($user, $order, $request);

        if ($fraudScore > 0.85) {
            Log::warning("Payment blocked by fraud detection", [
                'order_id' => $order->id,
                'email' => $user->email,
                'fraud_score' => $fraudScore,
                'ip_address' => $request->ip(),
                'amount' => $validated['amount'],
            ]);
            return response()->json(['message' => 'Payment could not be processed. Please contact support.'], 422);
        }

        // Step 2: Create payment intent via Stripe
        try {
            $stripeResponse = Http::post("{$this->stripeBaseUrl}/payment_intents", [
                'amount' => $amount,
                'currency' => $currency,
                'payment_method' => $validated['payment_method_id'],
                'customer' => $user->stripe_customer_id,
                'confirm' => true,
                'metadata' => [
                    'order_id' => $order->id,
                    'user_id' => $user->id,
                    'idempotency_key' => $idempotencyKey,
                ],
            ]);

            if ($stripeResponse->failed()) {
                Log::error("Stripe payment intent creation failed", [
                    'order_id' => $order->id,
                    'email' => $user->email,
                    'stripe_status' => $stripeResponse->status(),
                    'stripe_error' => $stripeResponse->json('error.message'),
                    'amount' => $validated['amount'],
                ]);
                return response()->json([
                    'message' => 'Payment processing failed',
                    'error' => $stripeResponse->json('error.message'),
                ], 402);
            }

            $stripeData = $stripeResponse->json();

            // Step 3: Record payment in our database
            $payment = DB::transaction(function () use ($order, $user, $validated, $stripeData, $currency, $idempotencyKey) {
                $payment = Payment::create([
                    'order_id' => $order->id,
                    'user_id' => $user->id,
                    'stripe_payment_intent_id' => $stripeData['id'],
                    'amount' => $validated['amount'],
                    'currency' => $currency,
                    'status' => $stripeData['status'],
                    'payment_method_id' => $validated['payment_method_id'],
                    'idempotency_key' => $idempotencyKey,
                ]);

                $order->update(['status' => 'paid', 'paid_at' => now()]);

                return $payment;
            });

            // Step 4: Optionally save payment method for future use
            if ($validated['save_method'] ?? false) {
                Http::post("{$this->stripeBaseUrl}/payment_methods/{$validated['payment_method_id']}/attach", [
                    'customer' => $user->stripe_customer_id,
                ]);
            }

            event(new PaymentCompleted($payment));

            // Step 5: Send payment confirmation email
            Http::post('https://api.sendgrid.com/v3/mail/send', [
                'to' => $user->email,
                'template_id' => config('services.sendgrid.payment_confirmation_template'),
                'dynamic_data' => [
                    'name' => $user->name,
                    'amount' => $validated['amount'],
                    'currency' => strtoupper($currency),
                    'order_id' => $order->id,
                ],
            ]);

            Log::info("Payment completed successfully for email: {$user->email}, amount: {$validated['amount']}, credit_card: {$validated['payment_method_id']}");

            return response()->json($payment, 201);

        } catch (\Exception $e) {
            Log::error("Payment processing exception", [
                'order_id' => $order->id,
                'email' => $user->email,
                'error' => $e->getMessage(),
                'amount' => $validated['amount'],
            ]);

            return response()->json(['message' => 'An error occurred during payment processing'], 500);
        }
    }

    /**
     * Show payment details.
     */
    public function show(Request $request, string $id): JsonResponse
    {
        $payment = Payment::with(['order', 'user'])->findOrFail($id);

        if ($request->user()->id !== $payment->user_id && $request->user()->role !== 'admin') {
            return response()->json(['message' => 'Forbidden'], 403);
        }

        // Fetch latest status from Stripe
        $stripePayment = Http::get("{$this->stripeBaseUrl}/payment_intents/{$payment->stripe_payment_intent_id}");

        if ($stripePayment->successful()) {
            $payment->stripe_status = $stripePayment->json('status');
        }

        Log::info("Payment details accessed", [
            'payment_id' => $payment->id,
            'email' => $request->user()->email,
        ]);

        return response()->json($payment);
    }

    /**
     * Process a full or partial refund.
     */
    public function refund(Request $request, string $id): JsonResponse
    {
        $payment = Payment::findOrFail($id);

        $validated = $request->validate([
            'amount' => 'sometimes|numeric|min:0.50',
            'reason' => 'required|in:duplicate,fraudulent,requested_by_customer,product_defective',
        ]);

        $refundAmount = $validated['amount'] ?? $payment->amount;
        $refundAmountCents = (int) ($refundAmount * 100);

        if ($refundAmount > $payment->amount) {
            return response()->json(['message' => 'Refund amount exceeds payment amount'], 422);
        }

        Log::info("Refund initiated", [
            'payment_id' => $payment->id,
            'refund_amount' => $refundAmount,
            'original_amount' => $payment->amount,
            'reason' => $validated['reason'],
            'admin_email' => $request->user()->email,
        ]);

        try {
            $refundResponse = Http::post("{$this->stripeBaseUrl}/refunds", [
                'payment_intent' => $payment->stripe_payment_intent_id,
                'amount' => $refundAmountCents,
                'reason' => $validated['reason'],
            ]);

            if ($refundResponse->failed()) {
                Log::error("Stripe refund failed", [
                    'payment_id' => $payment->id,
                    'stripe_error' => $refundResponse->json('error.message'),
                    'refund_amount' => $refundAmount,
                ]);
                return response()->json([
                    'message' => 'Refund processing failed',
                    'error' => $refundResponse->json('error.message'),
                ], 422);
            }

            $refundData = $refundResponse->json();

            DB::transaction(function () use ($payment, $refundAmount, $refundData, $validated) {
                $payment->update([
                    'refunded_amount' => $payment->refunded_amount + $refundAmount,
                    'status' => $refundAmount >= $payment->amount ? 'refunded' : 'partially_refunded',
                    'stripe_refund_id' => $refundData['id'],
                ]);

                $payment->order->update([
                    'status' => $refundAmount >= $payment->amount ? 'refunded' : 'partially_refunded',
                ]);

                $payment->refundHistory()->create([
                    'amount' => $refundAmount,
                    'reason' => $validated['reason'],
                    'stripe_refund_id' => $refundData['id'],
                    'processed_by' => auth()->id(),
                ]);
            });

            // Notify customer about the refund
            $customer = $payment->user;
            Http::post('https://api.sendgrid.com/v3/mail/send', [
                'to' => $customer->email,
                'template_id' => config('services.sendgrid.refund_template'),
                'dynamic_data' => [
                    'name' => $customer->name,
                    'refund_amount' => $refundAmount,
                    'order_id' => $payment->order_id,
                ],
            ]);

            event(new RefundProcessed($payment));

            Log::info("Refund processed successfully for email: {$customer->email}, amount: {$refundAmount}, payment_id: {$payment->id}");

            return response()->json($payment->fresh());

        } catch (\Exception $e) {
            Log::error("Refund processing exception", [
                'payment_id' => $payment->id,
                'error' => $e->getMessage(),
                'refund_amount' => $refundAmount,
            ]);

            return response()->json(['message' => 'An error occurred during refund processing'], 500);
        }
    }

    /**
     * Download payment receipt.
     */
    public function receipt(Request $request, string $id): JsonResponse
    {
        $payment = Payment::with('order.items')->findOrFail($id);

        if ($request->user()->id !== $payment->user_id) {
            return response()->json(['message' => 'Forbidden'], 403);
        }

        // Fetch charge details from Stripe for receipt URL
        $chargeResponse = Http::get("{$this->stripeBaseUrl}/charges", [
            'payment_intent' => $payment->stripe_payment_intent_id,
        ]);

        $receiptUrl = $chargeResponse->successful()
            ? $chargeResponse->json('data.0.receipt_url')
            : null;

        return response()->json([
            'payment' => $payment,
            'receipt_url' => $receiptUrl,
        ]);
    }

    /**
     * Capture an authorized payment (admin only).
     */
    public function capture(Request $request, string $id): JsonResponse
    {
        $payment = Payment::findOrFail($id);

        if ($payment->status !== 'requires_capture') {
            return response()->json(['message' => 'Payment is not in a capturable state'], 422);
        }

        $captureResponse = Http::post("{$this->stripeBaseUrl}/payment_intents/{$payment->stripe_payment_intent_id}/capture");

        if ($captureResponse->failed()) {
            Log::error("Payment capture failed", [
                'payment_id' => $payment->id,
                'stripe_error' => $captureResponse->json('error.message'),
            ]);
            return response()->json(['message' => 'Capture failed'], 422);
        }

        $payment->update(['status' => 'succeeded']);

        Log::info("Payment captured", ['payment_id' => $payment->id, 'amount' => $payment->amount]);

        return response()->json($payment);
    }

    /**
     * Check fraud risk score for a payment attempt.
     */
    private function checkFraudRisk(User $user, Order $order, Request $request): float
    {
        try {
            // Use Guzzle client for the fraud detection service
            $response = $this->guzzleClient->request('POST', "{$this->fraudServiceUrl}/api/v1/check", [
                'json' => [
                    'email' => $user->email,
                    'ip_address' => $request->ip(),
                    'amount' => $order->total,
                    'currency' => $order->currency,
                    'user_id' => $user->id,
                    'order_count' => $user->orders()->count(),
                    'account_age_days' => $user->created_at->diffInDays(now()),
                    'shipping_address' => $order->shippingAddress?->toArray(),
                ],
            ]);

            $result = json_decode($response->getBody()->getContents(), true);

            Log::info("Fraud check completed", [
                'user_id' => $user->id,
                'score' => $result['score'],
                'risk_level' => $result['risk_level'],
            ]);

            return $result['score'] ?? 0.0;

        } catch (\Exception $e) {
            Log::error("Fraud detection service unavailable", [
                'error' => $e->getMessage(),
                'email' => $user->email,
            ]);
            // Default to allowing the transaction if fraud service is down
            return 0.0;
        }
    }

    /**
     * List user's saved payment methods.
     */
    public function methods(Request $request): JsonResponse
    {
        $user = $request->user();

        if (!$user->stripe_customer_id) {
            return response()->json(['data' => []]);
        }

        $methods = Http::get("{$this->stripeBaseUrl}/payment_methods", [
            'customer' => $user->stripe_customer_id,
            'type' => 'card',
        ]);

        return response()->json($methods->json());
    }

    /**
     * Add a new payment method via Stripe.
     */
    public function addMethod(Request $request): JsonResponse
    {
        $validated = $request->validate([
            'payment_method_id' => 'required|string',
        ]);

        $user = $request->user();

        // Ensure customer exists in Stripe
        if (!$user->stripe_customer_id) {
            $customerResponse = Http::post("{$this->stripeBaseUrl}/customers", [
                'email' => $user->email,
                'name' => $user->name,
                'metadata' => ['user_id' => $user->id],
            ]);

            $user->update(['stripe_customer_id' => $customerResponse->json('id')]);
        }

        $attachResponse = Http::post("{$this->stripeBaseUrl}/payment_methods/{$validated['payment_method_id']}/attach", [
            'customer' => $user->stripe_customer_id,
        ]);

        if ($attachResponse->failed()) {
            Log::error("Failed to attach payment method", [
                'email' => $user->email,
                'error' => $attachResponse->json('error.message'),
            ]);
            return response()->json(['message' => 'Could not save payment method'], 422);
        }

        Log::info("Payment method added for email: {$user->email}, credit_card: {$validated['payment_method_id']}");

        return response()->json($attachResponse->json(), 201);
    }

    /**
     * Remove a saved payment method.
     */
    public function removeMethod(Request $request, string $id): JsonResponse
    {
        $detachResponse = Http::post("{$this->stripeBaseUrl}/payment_methods/{$id}/detach");

        if ($detachResponse->failed()) {
            return response()->json(['message' => 'Could not remove payment method'], 422);
        }

        Log::info("Payment method removed", ['user_email' => $request->user()->email, 'method_id' => $id]);

        return response()->json(['message' => 'Payment method removed']);
    }

    /**
     * Admin: list all payments with filters.
     */
    public function adminIndex(Request $request): JsonResponse
    {
        $payments = Payment::with(['user', 'order'])
            ->when($request->input('status'), fn ($q, $s) => $q->where('status', $s))
            ->when($request->input('from'), fn ($q, $d) => $q->where('created_at', '>=', $d))
            ->when($request->input('to'), fn ($q, $d) => $q->where('created_at', '<=', $d))
            ->when($request->input('min_amount'), fn ($q, $a) => $q->where('amount', '>=', $a))
            ->orderBy('created_at', 'desc')
            ->paginate(50);

        return response()->json($payments);
    }

    /**
     * Admin: export payments as CSV.
     */
    public function exportCsv(Request $request): JsonResponse
    {
        Log::info("Payment CSV export requested by admin: {$request->user()->email}");

        $payments = Payment::with(['user', 'order'])
            ->when($request->input('from'), fn ($q, $d) => $q->where('created_at', '>=', $d))
            ->when($request->input('to'), fn ($q, $d) => $q->where('created_at', '<=', $d))
            ->get();

        // In production this would stream a CSV download — simplified for API response
        return response()->json([
            'count' => $payments->count(),
            'download_url' => url("/api/admin/payments/export/{$request->query('from')}/{$request->query('to')}"),
        ]);
    }
}
