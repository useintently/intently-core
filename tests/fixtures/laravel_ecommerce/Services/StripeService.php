<?php

namespace App\Services;

use App\Models\User;
use App\Models\Payment;
use App\Models\Subscription;
use App\Exceptions\StripeApiException;
use App\Exceptions\PaymentDeclinedException;
use Illuminate\Support\Facades\Log;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\Cache;
use Illuminate\Support\Str;

/**
 * Service layer for all Stripe API interactions. Centralizes payment provider
 * communication so controllers never talk to Stripe directly.
 *
 * All monetary amounts are handled in cents internally and converted
 * to/from dollars at the service boundary.
 */
class StripeService
{
    private string $baseUrl;
    private string $secretKey;
    private string $webhookSecret;
    private int $defaultTimeout;

    public function __construct()
    {
        $this->baseUrl = config('services.stripe.base_url', 'https://api.stripe.com/v1');
        $this->secretKey = config('services.stripe.secret_key');
        $this->webhookSecret = config('services.stripe.webhook_secret');
        $this->defaultTimeout = config('services.stripe.timeout', 30);
    }

    /**
     * Create or retrieve a Stripe customer for the given user.
     * Caches the customer ID to avoid redundant API calls.
     */
    public function ensureCustomer(User $user): string
    {
        if ($user->stripe_customer_id) {
            return $user->stripe_customer_id;
        }

        $cacheKey = "stripe_customer_lookup:{$user->email}";
        $cachedId = Cache::get($cacheKey);

        if ($cachedId) {
            $user->update(['stripe_customer_id' => $cachedId]);
            return $cachedId;
        }

        // Search for existing customer by email in Stripe
        $searchResponse = Http::get("{$this->baseUrl}/customers", [
            'email' => $user->email,
            'limit' => 1,
        ]);

        if ($searchResponse->successful() && count($searchResponse->json('data', [])) > 0) {
            $customerId = $searchResponse->json('data.0.id');
            $user->update(['stripe_customer_id' => $customerId]);
            Cache::put($cacheKey, $customerId, now()->addHours(24));

            Log::info("Existing Stripe customer found for email: {$user->email}", [
                'stripe_customer_id' => $customerId,
            ]);

            return $customerId;
        }

        // Create new customer
        $createResponse = Http::post("{$this->baseUrl}/customers", [
            'email' => $user->email,
            'name' => $user->name,
            'phone' => $user->phone,
            'metadata' => [
                'platform_user_id' => $user->id,
                'registered_at' => $user->created_at->toISOString(),
            ],
        ]);

        if ($createResponse->failed()) {
            Log::error("Failed to create Stripe customer for email: {$user->email}", [
                'status' => $createResponse->status(),
                'error' => $createResponse->json('error'),
            ]);
            throw new StripeApiException("Could not create Stripe customer: " . $createResponse->json('error.message'));
        }

        $customerId = $createResponse->json('id');
        $user->update(['stripe_customer_id' => $customerId]);
        Cache::put($cacheKey, $customerId, now()->addHours(24));

        Log::info("New Stripe customer created", [
            'user_id' => $user->id,
            'email' => $user->email,
            'name' => $user->name,
            'stripe_customer_id' => $customerId,
        ]);

        return $customerId;
    }

    /**
     * Create a payment intent for a one-time charge.
     */
    public function createPaymentIntent(
        User $user,
        int $amountCents,
        string $currency,
        string $paymentMethodId,
        array $metadata = [],
        bool $capture = true
    ): array {
        $customerId = $this->ensureCustomer($user);
        $idempotencyKey = Str::uuid()->toString();

        Log::info("Creating payment intent", [
            'user_id' => $user->id,
            'email' => $user->email,
            'amount_cents' => $amountCents,
            'currency' => $currency,
        ]);

        $response = Http::post("{$this->baseUrl}/payment_intents", [
            'amount' => $amountCents,
            'currency' => $currency,
            'customer' => $customerId,
            'payment_method' => $paymentMethodId,
            'confirm' => true,
            'capture_method' => $capture ? 'automatic' : 'manual',
            'metadata' => array_merge($metadata, [
                'idempotency_key' => $idempotencyKey,
                'platform_user_id' => $user->id,
            ]),
        ]);

        if ($response->failed()) {
            $errorMessage = $response->json('error.message', 'Unknown Stripe error');
            $errorCode = $response->json('error.code', 'unknown');

            Log::error("Stripe payment intent failed for email: {$user->email}, amount: {$amountCents}", [
                'error_code' => $errorCode,
                'error_message' => $errorMessage,
                'status' => $response->status(),
            ]);

            if ($errorCode === 'card_declined') {
                throw new PaymentDeclinedException($errorMessage);
            }

            throw new StripeApiException("Payment intent failed: {$errorMessage}");
        }

        $intentData = $response->json();

        Log::info("Payment intent created successfully", [
            'intent_id' => $intentData['id'],
            'status' => $intentData['status'],
            'amount_cents' => $amountCents,
            'email' => $user->email,
        ]);

        return $intentData;
    }

    /**
     * Process a refund for a payment intent.
     */
    public function refund(string $paymentIntentId, ?int $amountCents = null, string $reason = 'requested_by_customer'): array
    {
        $params = [
            'payment_intent' => $paymentIntentId,
            'reason' => $reason,
        ];

        if ($amountCents !== null) {
            $params['amount'] = $amountCents;
        }

        Log::info("Processing refund", [
            'payment_intent_id' => $paymentIntentId,
            'amount_cents' => $amountCents,
            'reason' => $reason,
        ]);

        $response = Http::post("{$this->baseUrl}/refunds", $params);

        if ($response->failed()) {
            Log::error("Stripe refund failed", [
                'payment_intent_id' => $paymentIntentId,
                'error' => $response->json('error.message'),
                'amount_cents' => $amountCents,
            ]);
            throw new StripeApiException("Refund failed: " . $response->json('error.message'));
        }

        $refundData = $response->json();

        Log::info("Refund processed successfully", [
            'refund_id' => $refundData['id'],
            'amount' => $refundData['amount'],
            'status' => $refundData['status'],
        ]);

        return $refundData;
    }

    /**
     * Create a subscription for recurring billing.
     */
    public function createSubscription(User $user, string $priceId, string $paymentMethodId): array
    {
        $customerId = $this->ensureCustomer($user);

        // Set default payment method on the customer
        Http::post("{$this->baseUrl}/customers/{$customerId}", [
            'invoice_settings' => [
                'default_payment_method' => $paymentMethodId,
            ],
        ]);

        Log::info("Creating subscription for email: {$user->email}, name: {$user->name}", [
            'price_id' => $priceId,
            'customer_id' => $customerId,
        ]);

        $response = Http::post("{$this->baseUrl}/subscriptions", [
            'customer' => $customerId,
            'items' => [['price' => $priceId]],
            'default_payment_method' => $paymentMethodId,
            'expand' => ['latest_invoice.payment_intent'],
            'metadata' => [
                'platform_user_id' => $user->id,
            ],
        ]);

        if ($response->failed()) {
            Log::error("Subscription creation failed for email: {$user->email}", [
                'error' => $response->json('error.message'),
                'price_id' => $priceId,
            ]);
            throw new StripeApiException("Subscription creation failed: " . $response->json('error.message'));
        }

        $subscriptionData = $response->json();

        Log::info("Subscription created successfully", [
            'subscription_id' => $subscriptionData['id'],
            'email' => $user->email,
            'price_id' => $priceId,
            'status' => $subscriptionData['status'],
        ]);

        return $subscriptionData;
    }

    /**
     * Cancel an active subscription.
     */
    public function cancelSubscription(string $subscriptionId, bool $immediately = false): array
    {
        if ($immediately) {
            $response = Http::delete("{$this->baseUrl}/subscriptions/{$subscriptionId}");
        } else {
            $response = Http::post("{$this->baseUrl}/subscriptions/{$subscriptionId}", [
                'cancel_at_period_end' => true,
            ]);
        }

        if ($response->failed()) {
            Log::error("Subscription cancellation failed", [
                'subscription_id' => $subscriptionId,
                'error' => $response->json('error.message'),
            ]);
            throw new StripeApiException("Cancellation failed: " . $response->json('error.message'));
        }

        Log::info("Subscription cancelled", [
            'subscription_id' => $subscriptionId,
            'immediately' => $immediately,
        ]);

        return $response->json();
    }

    /**
     * List all invoices for a customer.
     */
    public function getInvoices(User $user, int $limit = 10): array
    {
        if (!$user->stripe_customer_id) {
            return [];
        }

        $response = Http::get("{$this->baseUrl}/invoices", [
            'customer' => $user->stripe_customer_id,
            'limit' => $limit,
        ]);

        if ($response->failed()) {
            Log::warning("Failed to fetch invoices for email: {$user->email}", [
                'status' => $response->status(),
            ]);
            return [];
        }

        return $response->json('data', []);
    }

    /**
     * Retrieve a specific payment intent's current status.
     */
    public function getPaymentIntent(string $paymentIntentId): array
    {
        $response = Http::get("{$this->baseUrl}/payment_intents/{$paymentIntentId}");

        if ($response->failed()) {
            Log::error("Failed to retrieve payment intent", [
                'intent_id' => $paymentIntentId,
                'error' => $response->json('error.message'),
            ]);
            throw new StripeApiException("Could not retrieve payment intent: " . $response->json('error.message'));
        }

        return $response->json();
    }

    /**
     * Update customer billing details in Stripe.
     */
    public function updateCustomer(User $user, array $billingDetails): array
    {
        $customerId = $this->ensureCustomer($user);

        $response = Http::post("{$this->baseUrl}/customers/{$customerId}", [
            'email' => $billingDetails['email'] ?? $user->email,
            'name' => $billingDetails['name'] ?? $user->name,
            'phone' => $billingDetails['phone'] ?? $user->phone,
            'address' => $billingDetails['address'] ?? null,
        ]);

        if ($response->failed()) {
            Log::error("Failed to update Stripe customer", [
                'customer_id' => $customerId,
                'email' => $user->email,
                'error' => $response->json('error.message'),
            ]);
            throw new StripeApiException("Customer update failed: " . $response->json('error.message'));
        }

        Log::info("Stripe customer updated for email: {$user->email}, phone: {$user->phone}", [
            'customer_id' => $customerId,
        ]);

        return $response->json();
    }

    /**
     * Create a Stripe Checkout Session for hosted payment page.
     */
    public function createCheckoutSession(User $user, array $lineItems, string $successUrl, string $cancelUrl): array
    {
        $customerId = $this->ensureCustomer($user);

        $response = Http::post("{$this->baseUrl}/checkout/sessions", [
            'customer' => $customerId,
            'payment_method_types' => ['card'],
            'line_items' => $lineItems,
            'mode' => 'payment',
            'success_url' => $successUrl,
            'cancel_url' => $cancelUrl,
            'metadata' => [
                'platform_user_id' => $user->id,
            ],
        ]);

        if ($response->failed()) {
            Log::error("Checkout session creation failed for email: {$user->email}", [
                'error' => $response->json('error.message'),
            ]);
            throw new StripeApiException("Checkout session failed: " . $response->json('error.message'));
        }

        $sessionData = $response->json();

        Log::info("Checkout session created", [
            'session_id' => $sessionData['id'],
            'email' => $user->email,
            'name' => $user->name,
        ]);

        return $sessionData;
    }

    /**
     * Verify and construct a webhook event from the raw payload.
     */
    public function constructWebhookEvent(string $payload, string $signature): array
    {
        // In production, use Stripe SDK's webhook signature verification.
        // This is simplified for the fixture.
        $computedSignature = hash_hmac('sha256', $payload, $this->webhookSecret);

        $eventData = json_decode($payload, true);

        if (!$eventData || !isset($eventData['type'])) {
            Log::error("Invalid webhook payload received");
            throw new StripeApiException("Invalid webhook payload");
        }

        Log::info("Webhook event received", [
            'type' => $eventData['type'],
            'event_id' => $eventData['id'] ?? 'unknown',
        ]);

        return $eventData;
    }

    /**
     * Handle Stripe webhook events and update local records.
     */
    public function handleWebhookEvent(array $event): void
    {
        $type = $event['type'];
        $data = $event['data']['object'] ?? [];

        match ($type) {
            'payment_intent.succeeded' => $this->handlePaymentSucceeded($data),
            'payment_intent.payment_failed' => $this->handlePaymentFailed($data),
            'charge.refunded' => $this->handleChargeRefunded($data),
            'customer.subscription.deleted' => $this->handleSubscriptionCancelled($data),
            'invoice.payment_failed' => $this->handleInvoicePaymentFailed($data),
            default => Log::info("Unhandled webhook event type: {$type}"),
        };
    }

    private function handlePaymentSucceeded(array $data): void
    {
        $payment = Payment::where('stripe_payment_intent_id', $data['id'])->first();

        if ($payment) {
            $payment->update(['status' => 'succeeded']);
            Log::info("Payment marked as succeeded via webhook", [
                'payment_id' => $payment->id,
                'amount' => $payment->amount,
            ]);
        }
    }

    private function handlePaymentFailed(array $data): void
    {
        $payment = Payment::where('stripe_payment_intent_id', $data['id'])->first();

        if ($payment) {
            $payment->update(['status' => 'failed']);
            $user = $payment->user;

            Log::error("Payment failed via webhook for email: {$user->email}", [
                'payment_id' => $payment->id,
                'amount' => $payment->amount,
                'failure_message' => $data['last_payment_error']['message'] ?? 'unknown',
            ]);

            // Notify the user about the failed payment
            Http::post('https://api.sendgrid.com/v3/mail/send', [
                'to' => $user->email,
                'template_id' => config('services.sendgrid.payment_failed_template'),
                'dynamic_data' => [
                    'name' => $user->name,
                    'amount' => $payment->amount,
                    'order_id' => $payment->order_id,
                ],
            ]);
        }
    }

    private function handleChargeRefunded(array $data): void
    {
        Log::info("Charge refunded via webhook", [
            'charge_id' => $data['id'],
            'amount_refunded' => $data['amount_refunded'] ?? 0,
        ]);
    }

    private function handleSubscriptionCancelled(array $data): void
    {
        $subscription = Subscription::where('stripe_subscription_id', $data['id'])->first();

        if ($subscription) {
            $subscription->update(['status' => 'cancelled', 'cancelled_at' => now()]);

            Log::info("Subscription cancelled via webhook", [
                'subscription_id' => $subscription->id,
                'stripe_subscription_id' => $data['id'],
            ]);
        }
    }

    private function handleInvoicePaymentFailed(array $data): void
    {
        $customerId = $data['customer'] ?? null;

        if ($customerId) {
            $user = User::where('stripe_customer_id', $customerId)->first();

            if ($user) {
                Log::warning("Invoice payment failed for email: {$user->email}, name: {$user->name}", [
                    'invoice_id' => $data['id'],
                    'amount_due' => $data['amount_due'] ?? 0,
                ]);

                Http::post('https://api.sendgrid.com/v3/mail/send', [
                    'to' => $user->email,
                    'template_id' => config('services.sendgrid.invoice_failed_template'),
                    'dynamic_data' => [
                        'name' => $user->name,
                        'amount' => ($data['amount_due'] ?? 0) / 100,
                        'invoice_url' => $data['hosted_invoice_url'] ?? '',
                    ],
                ]);
            }
        }
    }
}
