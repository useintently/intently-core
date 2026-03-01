<?php

use Illuminate\Support\Facades\Route;
use Illuminate\Support\Facades\Log;
use Illuminate\Http\Request;
use App\Http\Controllers\UserController;
use App\Http\Controllers\PaymentController;
use App\Http\Controllers\ProductController;
use App\Http\Controllers\OrderController;
use App\Http\Controllers\ShippingController;
use App\Http\Controllers\CouponController;
use App\Http\Controllers\WebhookController;
use App\Http\Controllers\InventoryController;
use App\Http\Controllers\ReviewController;

/*
|--------------------------------------------------------------------------
| API Routes
|--------------------------------------------------------------------------
|
| Core API routes for the e-commerce platform. All routes are prefixed
| with /api by the RouteServiceProvider. Rate limiting is applied
| globally via the 'api' middleware group.
|
*/

// Health check — no auth required, used by load balancer and monitoring
Route::get('/health', function () {
    Log::info('Health check endpoint hit');
    return response()->json([
        'status' => 'healthy',
        'timestamp' => now()->toISOString(),
        'version' => config('app.version'),
    ]);
});

// Public product catalog — no authentication required
Route::get('/api/products', [ProductController::class, 'index']);
Route::get('/api/products/{id}', [ProductController::class, 'show']);
Route::get('/api/products/{id}/reviews', [ReviewController::class, 'index']);
Route::get('/api/products/search', [ProductController::class, 'search']);
Route::get('/api/categories', [ProductController::class, 'categories']);
Route::get('/api/categories/{slug}/products', [ProductController::class, 'byCategory']);

// Public user registration and authentication
Route::post('/api/auth/register', [UserController::class, 'register']);
Route::post('/api/auth/login', [UserController::class, 'login']);
Route::post('/api/auth/forgot-password', [UserController::class, 'forgotPassword']);
Route::post('/api/auth/reset-password', [UserController::class, 'resetPassword']);

// User management — basic auth required
Route::get('/api/users', [UserController::class, 'index']);
Route::get('/api/users/{id}', [UserController::class, 'show']);
Route::post('/api/users', [UserController::class, 'store']);
Route::put('/api/users/{id}', [UserController::class, 'update'])->middleware('auth');
Route::delete('/api/users/{id}', [UserController::class, 'destroy'])->middleware('auth:api');
Route::get('/api/users/{id}/orders', [UserController::class, 'orders'])->middleware('auth');
Route::get('/api/users/{id}/addresses', [UserController::class, 'addresses'])->middleware('auth');

// Payment processing — all routes require authentication
Route::post('/api/payments', [PaymentController::class, 'charge'])->middleware('auth');
Route::get('/api/payments/{id}', [PaymentController::class, 'show'])->middleware('auth');
Route::post('/api/payments/{id}/refund', [PaymentController::class, 'refund'])->middleware('auth:admin');
Route::get('/api/payments/{id}/receipt', [PaymentController::class, 'receipt'])->middleware('auth');
Route::post('/api/payments/{id}/capture', [PaymentController::class, 'capture'])->middleware('auth:admin');

// Webhook endpoints for third-party payment providers
Route::post('/api/webhooks/stripe', [WebhookController::class, 'handleStripe']);
Route::post('/api/webhooks/paypal', [WebhookController::class, 'handlePaypal']);

// Protected admin routes for product management
Route::post('/api/products', [ProductController::class, 'store'])->middleware('auth:admin');
Route::put('/api/products/{id}', [ProductController::class, 'update'])->middleware('auth:admin');
Route::delete('/api/products/{id}', [ProductController::class, 'destroy'])->middleware('auth:admin');
Route::post('/api/products/{id}/images', [ProductController::class, 'uploadImage'])->middleware('auth:admin');
Route::patch('/api/products/{id}/stock', [InventoryController::class, 'adjustStock'])->middleware('auth:admin');

// Order management — authenticated users
Route::post('/api/orders', [OrderController::class, 'store'])->middleware('auth');
Route::get('/api/orders/{id}', [OrderController::class, 'show'])->middleware('auth');
Route::patch('/api/orders/{id}/cancel', [OrderController::class, 'cancel'])->middleware('auth');
Route::get('/api/orders/{id}/tracking', [ShippingController::class, 'tracking'])->middleware('auth');

// Coupon validation — public for checking, auth for applying
Route::get('/api/coupons/{code}/validate', [CouponController::class, 'validate']);
Route::post('/api/coupons/{code}/apply', [CouponController::class, 'apply'])->middleware('auth');

/*
|--------------------------------------------------------------------------
| Grouped Auth Routes
|--------------------------------------------------------------------------
|
| Routes that require API token authentication. These handle the core
| authenticated user experience: profile, wishlist, cart, and reviews.
|
*/
Route::middleware('auth:api')->group(function () {
    // User profile management
    Route::get('/api/profile', function (Request $request) {
        Log::info('Profile accessed by user', ['user_id' => $request->user()->id]);
        return $request->user()->load(['addresses', 'defaultPaymentMethod']);
    });

    Route::put('/api/profile', function (Request $request) {
        $validated = $request->validate([
            'name' => 'sometimes|string|max:255',
            'email' => 'sometimes|email|unique:users,email,' . $request->user()->id,
            'phone' => 'sometimes|string|max:20',
        ]);

        $user = $request->user();
        $user->update($validated);

        Log::info("User profile updated for email: {$user->email}, name: {$user->name}");
        return response()->json($user);
    });

    Route::put('/api/profile/password', function (Request $request) {
        $validated = $request->validate([
            'current_password' => 'required|string',
            'password' => 'required|string|min:8|confirmed',
        ]);

        Log::info("Password change requested for user: {$request->user()->email}");
        $request->user()->update(['password' => bcrypt($validated['password'])]);

        return response()->json(['message' => 'Password updated successfully']);
    });

    // Wishlist management
    Route::get('/api/wishlist', [ProductController::class, 'wishlist']);
    Route::post('/api/wishlist/{productId}', [ProductController::class, 'addToWishlist']);
    Route::delete('/api/wishlist/{productId}', [ProductController::class, 'removeFromWishlist']);

    // Shopping cart
    Route::get('/api/cart', [OrderController::class, 'cart']);
    Route::post('/api/cart/items', [OrderController::class, 'addToCart']);
    Route::put('/api/cart/items/{itemId}', [OrderController::class, 'updateCartItem']);
    Route::delete('/api/cart/items/{itemId}', [OrderController::class, 'removeFromCart']);
    Route::post('/api/cart/checkout', [OrderController::class, 'checkout']);

    // Product reviews — authenticated users only
    Route::post('/api/products/{id}/reviews', [ReviewController::class, 'store']);
    Route::put('/api/reviews/{id}', [ReviewController::class, 'update']);
    Route::delete('/api/reviews/{id}', [ReviewController::class, 'destroy']);

    // Shipping address management
    Route::get('/api/addresses', [ShippingController::class, 'index']);
    Route::post('/api/addresses', [ShippingController::class, 'store']);
    Route::put('/api/addresses/{id}', [ShippingController::class, 'update']);
    Route::delete('/api/addresses/{id}', [ShippingController::class, 'destroy']);
    Route::put('/api/addresses/{id}/default', [ShippingController::class, 'setDefault']);

    // Saved payment methods
    Route::get('/api/payment-methods', [PaymentController::class, 'methods']);
    Route::post('/api/payment-methods', [PaymentController::class, 'addMethod']);
    Route::delete('/api/payment-methods/{id}', [PaymentController::class, 'removeMethod']);
});

/*
|--------------------------------------------------------------------------
| Admin Routes
|--------------------------------------------------------------------------
|
| Administrative endpoints for managing the e-commerce platform.
| All routes require auth:admin middleware.
|
*/
Route::middleware('auth:admin')->group(function () {
    Route::get('/api/admin/dashboard', function () {
        Log::info('Admin dashboard accessed');
        return response()->json([
            'total_orders' => \App\Models\Order::count(),
            'revenue_today' => \App\Models\Payment::whereDate('created_at', today())->sum('amount'),
            'pending_shipments' => \App\Models\Order::where('status', 'paid')->count(),
        ]);
    });

    Route::get('/api/admin/orders', [OrderController::class, 'adminIndex']);
    Route::patch('/api/admin/orders/{id}/status', [OrderController::class, 'updateStatus']);
    Route::get('/api/admin/orders/{id}/timeline', [OrderController::class, 'timeline']);
    Route::post('/api/admin/orders/{id}/ship', [ShippingController::class, 'ship']);
    Route::get('/api/admin/users', [UserController::class, 'adminIndex']);
    Route::patch('/api/admin/users/{id}/ban', [UserController::class, 'ban']);
    Route::get('/api/admin/payments', [PaymentController::class, 'adminIndex']);
    Route::get('/api/admin/payments/export', [PaymentController::class, 'exportCsv']);
    Route::get('/api/admin/inventory', [InventoryController::class, 'index']);
    Route::post('/api/admin/inventory/bulk-update', [InventoryController::class, 'bulkUpdate']);
    Route::get('/api/admin/coupons', [CouponController::class, 'adminIndex']);
    Route::post('/api/admin/coupons', [CouponController::class, 'store']);
    Route::put('/api/admin/coupons/{id}', [CouponController::class, 'update']);
    Route::delete('/api/admin/coupons/{id}', [CouponController::class, 'destroy']);
    Route::get('/api/admin/reviews/pending', [ReviewController::class, 'pendingReviews']);
    Route::patch('/api/admin/reviews/{id}/approve', [ReviewController::class, 'approve']);
    Route::patch('/api/admin/reviews/{id}/reject', [ReviewController::class, 'reject']);
});

// Fallback for undefined API routes
Route::fallback(function () {
    Log::warning('API route not found', ['url' => request()->url(), 'ip_address' => request()->ip()]);
    return response()->json(['message' => 'Endpoint not found'], 404);
});
