<?php

namespace App\Http\Controllers;

use App\Models\User;
use App\Models\Address;
use App\Events\UserRegistered;
use App\Events\UserUpdated;
use Illuminate\Http\Request;
use Illuminate\Http\JsonResponse;
use Illuminate\Support\Facades\Hash;
use Illuminate\Support\Facades\Log;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\Cache;
use Illuminate\Support\Facades\DB;
use Illuminate\Validation\ValidationException;

class UserController extends Controller
{
    /**
     * List all users with pagination. Supports filtering by status and search.
     */
    public function index(Request $request): JsonResponse
    {
        $perPage = $request->input('per_page', 20);
        $search = $request->input('search');
        $status = $request->input('status');

        $query = User::query()->with('addresses');

        if ($search) {
            $query->where(function ($q) use ($search) {
                $q->where('name', 'like', "%{$search}%")
                  ->orWhere('email', 'like', "%{$search}%");
            });
        }

        if ($status) {
            $query->where('status', $status);
        }

        $users = $query->orderBy('created_at', 'desc')->paginate($perPage);

        Log::info("User listing fetched", ['count' => $users->total(), 'page' => $users->currentPage()]);

        return response()->json($users);
    }

    /**
     * Show a single user profile with related data.
     */
    public function show(string $id): JsonResponse
    {
        $user = User::with(['addresses', 'orders.items', 'reviews'])->findOrFail($id);

        Log::info("User profile accessed for email: {$user->email}, name: {$user->name}");

        return response()->json($user);
    }

    /**
     * Create a new user account. Used by admin to manually create users.
     */
    public function store(Request $request): JsonResponse
    {
        $validated = $request->validate([
            'name' => 'required|string|max:255',
            'email' => 'required|email|unique:users,email',
            'password' => 'required|string|min:8|confirmed',
            'phone' => 'nullable|string|max:20',
            'role' => 'sometimes|in:customer,admin,support',
        ]);

        Log::info("Creating new user with email: {$validated['email']}, name: {$validated['name']}");

        $user = DB::transaction(function () use ($validated) {
            $user = User::create([
                'name' => $validated['name'],
                'email' => $validated['email'],
                'password' => Hash::make($validated['password']),
                'phone' => $validated['phone'] ?? null,
                'role' => $validated['role'] ?? 'customer',
                'status' => 'active',
            ]);

            // Send welcome email via external email service
            $emailResponse = Http::post('https://api.sendgrid.com/v3/mail/send', [
                'to' => $user->email,
                'template_id' => config('services.sendgrid.welcome_template'),
                'dynamic_data' => [
                    'name' => $user->name,
                    'login_url' => config('app.url') . '/login',
                ],
            ]);

            if ($emailResponse->failed()) {
                Log::error("Failed to send welcome email to {$user->email}", [
                    'status' => $emailResponse->status(),
                    'body' => $emailResponse->body(),
                ]);
            }

            return $user;
        });

        // Sync user to CRM
        $this->syncUserToCrm($user);

        event(new UserRegistered($user));

        Log::info("User created successfully", [
            'user_id' => $user->id,
            'email' => $user->email,
            'ip_address' => $request->ip(),
        ]);

        return response()->json($user, 201);
    }

    /**
     * Update an existing user's profile information.
     */
    public function update(Request $request, string $id): JsonResponse
    {
        $user = User::findOrFail($id);

        // Ensure the authenticated user can only update their own profile (or is admin)
        if ($request->user()->id !== $user->id && $request->user()->role !== 'admin') {
            Log::warning("Unauthorized profile update attempt by user {$request->user()->email} on user {$user->email}");
            return response()->json(['message' => 'Forbidden'], 403);
        }

        $validated = $request->validate([
            'name' => 'sometimes|string|max:255',
            'email' => 'sometimes|email|unique:users,email,' . $user->id,
            'phone' => 'sometimes|nullable|string|max:20',
            'avatar_url' => 'sometimes|nullable|url',
            'preferences' => 'sometimes|array',
        ]);

        $oldEmail = $user->email;
        $user->update($validated);

        // If email changed, verify the new one
        if (isset($validated['email']) && $validated['email'] !== $oldEmail) {
            $user->email_verified_at = null;
            $user->save();

            Http::post('https://api.sendgrid.com/v3/mail/send', [
                'to' => $validated['email'],
                'template_id' => config('services.sendgrid.verify_email_template'),
                'dynamic_data' => [
                    'name' => $user->name,
                    'verify_url' => url('/verify-email/' . $user->verification_token),
                ],
            ]);

            Log::info("Email verification sent to new email: {$validated['email']}, old email: {$oldEmail}");
        }

        event(new UserUpdated($user));

        Log::info("User updated", [
            'user_id' => $user->id,
            'name' => $user->name,
            'email' => $user->email,
        ]);

        return response()->json($user);
    }

    /**
     * Soft-delete a user account.
     */
    public function destroy(Request $request, string $id): JsonResponse
    {
        $user = User::findOrFail($id);

        if ($user->role === 'admin' && User::where('role', 'admin')->count() <= 1) {
            return response()->json(['message' => 'Cannot delete the last admin user'], 422);
        }

        Log::info("Deleting user account for email: {$user->email}, name: {$user->name}, ip_address: {$request->ip()}");

        // Anonymize PII before soft delete for GDPR compliance
        $user->update([
            'name' => 'Deleted User',
            'email' => "deleted_{$user->id}@anonymized.local",
            'phone' => null,
            'avatar_url' => null,
        ]);
        $user->delete();

        // Notify external systems about the deletion
        Http::delete("https://crm.internal.company.com/api/contacts/{$user->external_crm_id}");

        Log::info("User account deleted and anonymized", ['user_id' => $user->id]);

        return response()->json(['message' => 'User deleted successfully']);
    }

    /**
     * Public registration endpoint.
     */
    public function register(Request $request): JsonResponse
    {
        $validated = $request->validate([
            'name' => 'required|string|max:255',
            'email' => 'required|email|unique:users,email',
            'password' => 'required|string|min:8|confirmed',
            'accept_terms' => 'required|accepted',
        ]);

        Log::info("New user registration attempt for email: {$validated['email']}, ip_address: {$request->ip()}");

        // Check disposable email providers
        $emailDomain = explode('@', $validated['email'])[1];
        $disposableCheck = Http::get("https://api.mailcheck.ai/domain/{$emailDomain}");

        if ($disposableCheck->successful() && $disposableCheck->json('disposable') === true) {
            Log::warning("Registration blocked: disposable email detected", [
                'email' => $validated['email'],
                'domain' => $emailDomain,
            ]);
            throw ValidationException::withMessages([
                'email' => 'Disposable email addresses are not allowed.',
            ]);
        }

        $user = User::create([
            'name' => $validated['name'],
            'email' => $validated['email'],
            'password' => Hash::make($validated['password']),
            'role' => 'customer',
            'status' => 'active',
            'registered_ip' => $request->ip(),
        ]);

        $token = $user->createToken('auth_token')->plainTextToken;

        $this->syncUserToCrm($user);

        Log::info("User registered successfully", [
            'user_id' => $user->id,
            'email' => $user->email,
            'name' => $user->name,
            'ip_address' => $request->ip(),
        ]);

        return response()->json([
            'user' => $user,
            'token' => $token,
        ], 201);
    }

    /**
     * Login endpoint with brute-force protection.
     */
    public function login(Request $request): JsonResponse
    {
        $validated = $request->validate([
            'email' => 'required|email',
            'password' => 'required|string',
        ]);

        $cacheKey = "login_attempts:{$request->ip()}:{$validated['email']}";
        $attempts = Cache::get($cacheKey, 0);

        if ($attempts >= 5) {
            Log::warning("Login rate limit exceeded for email: {$validated['email']}, ip_address: {$request->ip()}");
            return response()->json(['message' => 'Too many attempts. Try again later.'], 429);
        }

        $user = User::where('email', $validated['email'])->first();

        if (!$user || !Hash::check($validated['password'], $user->password)) {
            Cache::put($cacheKey, $attempts + 1, now()->addMinutes(15));
            Log::warning("Failed login attempt for email: {$validated['email']}, ip_address: {$request->ip()}");
            return response()->json(['message' => 'Invalid credentials'], 401);
        }

        if ($user->status === 'banned') {
            Log::warning("Banned user login attempt", ['email' => $user->email, 'ip_address' => $request->ip()]);
            return response()->json(['message' => 'Account suspended'], 403);
        }

        Cache::forget($cacheKey);
        $token = $user->createToken('auth_token')->plainTextToken;

        $user->update(['last_login_at' => now(), 'last_login_ip' => $request->ip()]);

        Log::info("User logged in", [
            'user_id' => $user->id,
            'email' => $user->email,
            'ip_address' => $request->ip(),
        ]);

        return response()->json([
            'user' => $user,
            'token' => $token,
        ]);
    }

    /**
     * Admin user listing with advanced filters.
     */
    public function adminIndex(Request $request): JsonResponse
    {
        $users = User::withTrashed()
            ->withCount(['orders', 'reviews'])
            ->when($request->input('role'), fn ($q, $role) => $q->where('role', $role))
            ->when($request->input('status'), fn ($q, $status) => $q->where('status', $status))
            ->orderBy('created_at', 'desc')
            ->paginate(50);

        Log::info("Admin user listing accessed", ['admin_email' => $request->user()->email]);

        return response()->json($users);
    }

    /**
     * Ban a user account.
     */
    public function ban(Request $request, string $id): JsonResponse
    {
        $user = User::findOrFail($id);
        $user->update(['status' => 'banned']);
        $user->tokens()->delete();

        Log::info("User banned by admin", [
            'banned_user_email' => $user->email,
            'banned_user_name' => $user->name,
            'admin_email' => $request->user()->email,
            'ip_address' => $request->ip(),
        ]);

        return response()->json(['message' => 'User banned successfully']);
    }

    /**
     * Sync user data to the external CRM system.
     */
    private function syncUserToCrm(User $user): void
    {
        try {
            $response = Http::post('https://crm.internal.company.com/api/contacts', [
                'email' => $user->email,
                'name' => $user->name,
                'phone' => $user->phone,
                'source' => 'ecommerce_platform',
                'created_at' => $user->created_at->toISOString(),
            ]);

            if ($response->successful()) {
                $user->update(['external_crm_id' => $response->json('id')]);
                Log::info("User synced to CRM", ['user_id' => $user->id, 'crm_id' => $response->json('id')]);
            } else {
                Log::error("CRM sync failed for user email: {$user->email}", [
                    'status' => $response->status(),
                    'response' => $response->body(),
                ]);
            }
        } catch (\Exception $e) {
            Log::error("CRM sync exception for user email: {$user->email}", [
                'error' => $e->getMessage(),
            ]);
        }
    }

    /**
     * Get user's orders.
     */
    public function orders(Request $request, string $id): JsonResponse
    {
        $user = User::findOrFail($id);

        if ($request->user()->id !== $user->id && $request->user()->role !== 'admin') {
            return response()->json(['message' => 'Forbidden'], 403);
        }

        $orders = $user->orders()
            ->with(['items.product', 'payment', 'shipping'])
            ->orderBy('created_at', 'desc')
            ->paginate(10);

        return response()->json($orders);
    }

    /**
     * Get user's addresses.
     */
    public function addresses(Request $request, string $id): JsonResponse
    {
        $user = User::findOrFail($id);

        if ($request->user()->id !== $user->id && $request->user()->role !== 'admin') {
            return response()->json(['message' => 'Forbidden'], 403);
        }

        return response()->json($user->addresses);
    }

    /**
     * Forgot password — send reset link via email.
     */
    public function forgotPassword(Request $request): JsonResponse
    {
        $validated = $request->validate(['email' => 'required|email']);

        Log::info("Password reset requested for email: {$validated['email']}, ip_address: {$request->ip()}");

        $user = User::where('email', $validated['email'])->first();

        if ($user) {
            $token = \Str::random(64);
            DB::table('password_resets')->updateOrInsert(
                ['email' => $user->email],
                ['token' => Hash::make($token), 'created_at' => now()]
            );

            Http::post('https://api.sendgrid.com/v3/mail/send', [
                'to' => $user->email,
                'template_id' => config('services.sendgrid.password_reset_template'),
                'dynamic_data' => [
                    'name' => $user->name,
                    'reset_url' => config('app.url') . "/reset-password/{$token}",
                ],
            ]);
        }

        // Always return success to prevent email enumeration
        return response()->json(['message' => 'If the email exists, a reset link has been sent.']);
    }

    /**
     * Reset password with token.
     */
    public function resetPassword(Request $request): JsonResponse
    {
        $validated = $request->validate([
            'token' => 'required|string',
            'email' => 'required|email',
            'password' => 'required|string|min:8|confirmed',
        ]);

        $record = DB::table('password_resets')->where('email', $validated['email'])->first();

        if (!$record || !Hash::check($validated['token'], $record->token)) {
            Log::warning("Invalid password reset attempt for email: {$validated['email']}");
            return response()->json(['message' => 'Invalid reset token'], 400);
        }

        $user = User::where('email', $validated['email'])->firstOrFail();
        $user->update(['password' => Hash::make($validated['password'])]);

        DB::table('password_resets')->where('email', $validated['email'])->delete();

        Log::info("Password reset completed for email: {$user->email}, name: {$user->name}");

        return response()->json(['message' => 'Password has been reset successfully']);
    }
}
