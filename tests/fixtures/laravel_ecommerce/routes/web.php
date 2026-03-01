<?php

use Illuminate\Support\Facades\Route;
use Illuminate\Support\Facades\Log;
use Illuminate\Http\Request;
use App\Http\Controllers\AuthController;
use App\Http\Controllers\DashboardController;
use App\Http\Controllers\PageController;
use App\Http\Controllers\CartController;
use App\Http\Controllers\CheckoutController;
use App\Http\Controllers\AccountController;
use App\Http\Controllers\ProductController;

/*
|--------------------------------------------------------------------------
| Web Routes
|--------------------------------------------------------------------------
|
| Server-rendered web routes for the e-commerce storefront. These routes
| return Blade views and handle form submissions with CSRF protection
| via the VerifyCsrfToken middleware applied by the web middleware group.
|
*/

// Public storefront pages
Route::get('/', function () {
    Log::info('Homepage visited', ['ip_address' => request()->ip()]);
    return view('welcome', [
        'featured' => \App\Models\Product::featured()->take(8)->get(),
        'categories' => \App\Models\Category::withCount('products')->get(),
    ]);
});

Route::get('/about', [PageController::class, 'about']);
Route::get('/contact', [PageController::class, 'contact']);
Route::post('/contact', [PageController::class, 'submitContact']);
Route::get('/terms', [PageController::class, 'terms']);
Route::get('/privacy', [PageController::class, 'privacy']);
Route::get('/faq', [PageController::class, 'faq']);

// Product browsing — public
Route::get('/shop', [ProductController::class, 'shopIndex']);
Route::get('/shop/{category}', [ProductController::class, 'shopByCategory']);
Route::get('/product/{slug}', [ProductController::class, 'shopShow']);
Route::get('/search', [ProductController::class, 'shopSearch']);

// Authentication routes
Route::get('/login', [AuthController::class, 'showLogin'])->name('login');
Route::post('/login', [AuthController::class, 'login']);
Route::get('/register', [AuthController::class, 'showRegister']);
Route::post('/register', [AuthController::class, 'register']);
Route::post('/logout', [AuthController::class, 'logout'])->middleware('auth');
Route::get('/forgot-password', [AuthController::class, 'showForgotPassword']);
Route::post('/forgot-password', [AuthController::class, 'sendResetLink']);
Route::get('/reset-password/{token}', [AuthController::class, 'showResetPassword']);
Route::post('/reset-password', [AuthController::class, 'resetPassword']);

// OAuth social login callbacks
Route::get('/auth/google/redirect', [AuthController::class, 'googleRedirect']);
Route::get('/auth/google/callback', function (Request $request) {
    Log::info("Google OAuth callback received for email: {$request->query('email')}");
    return app(AuthController::class)->googleCallback($request);
});

Route::get('/auth/github/redirect', [AuthController::class, 'githubRedirect']);
Route::get('/auth/github/callback', [AuthController::class, 'githubCallback']);

// Shopping cart — session-based, no auth required
Route::get('/cart', [CartController::class, 'show']);
Route::post('/cart/add', [CartController::class, 'add']);
Route::put('/cart/update/{itemId}', [CartController::class, 'update']);
Route::delete('/cart/remove/{itemId}', [CartController::class, 'remove']);
Route::post('/cart/apply-coupon', [CartController::class, 'applyCoupon']);

// Checkout — requires authentication
Route::get('/checkout', [CheckoutController::class, 'show'])->middleware('auth');
Route::post('/checkout', [CheckoutController::class, 'process'])->middleware('auth');
Route::get('/checkout/success/{orderId}', [CheckoutController::class, 'success'])->middleware('auth');
Route::get('/checkout/cancel', [CheckoutController::class, 'cancel'])->middleware('auth');

/*
|--------------------------------------------------------------------------
| Authenticated User Account Routes
|--------------------------------------------------------------------------
*/
Route::middleware('auth')->group(function () {
    // Dashboard
    Route::get('/dashboard', [DashboardController::class, 'index']);

    // Account management
    Route::get('/account', [AccountController::class, 'show']);
    Route::get('/account/edit', [AccountController::class, 'edit']);
    Route::put('/account', [AccountController::class, 'update']);
    Route::get('/account/password', [AccountController::class, 'showChangePassword']);
    Route::put('/account/password', [AccountController::class, 'changePassword']);

    // Order history
    Route::get('/orders', [AccountController::class, 'orders']);
    Route::get('/orders/{id}', [AccountController::class, 'orderDetail']);
    Route::get('/orders/{id}/invoice', [AccountController::class, 'downloadInvoice']);
    Route::post('/orders/{id}/return', [AccountController::class, 'requestReturn']);

    // Wishlist
    Route::get('/wishlist', [AccountController::class, 'wishlist']);
    Route::post('/wishlist/toggle/{productId}', [AccountController::class, 'toggleWishlist']);

    // Saved addresses
    Route::get('/addresses', [AccountController::class, 'addresses']);
    Route::post('/addresses', [AccountController::class, 'storeAddress']);
    Route::put('/addresses/{id}', [AccountController::class, 'updateAddress']);
    Route::delete('/addresses/{id}', [AccountController::class, 'deleteAddress']);

    // Product reviews
    Route::get('/reviews', [AccountController::class, 'reviews']);
    Route::get('/product/{slug}/review', [AccountController::class, 'showReviewForm']);
    Route::post('/product/{slug}/review', [AccountController::class, 'submitReview']);
});

// Admin panel — web-based admin interface
Route::middleware('auth:admin')->group(function () {
    Route::get('/admin', [DashboardController::class, 'adminDashboard']);
    Route::get('/admin/orders', [DashboardController::class, 'adminOrders']);
    Route::get('/admin/products', [DashboardController::class, 'adminProducts']);
    Route::get('/admin/users', [DashboardController::class, 'adminUsers']);
});

// Locale and currency switching
Route::get('/locale/{locale}', function (string $locale) {
    if (in_array($locale, ['en', 'es', 'pt', 'fr', 'de'])) {
        session(['locale' => $locale]);
        Log::info("Locale switched to {$locale} by user session");
    }
    return redirect()->back();
});

Route::get('/currency/{currency}', function (string $currency) {
    if (in_array($currency, ['USD', 'EUR', 'BRL', 'GBP'])) {
        session(['currency' => $currency]);
        Log::info("Currency switched to {$currency}");
    }
    return redirect()->back();
});

// Sitemap and robots
Route::get('/sitemap.xml', [PageController::class, 'sitemap']);
