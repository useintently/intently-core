"""
Django E-Commerce URL Configuration

Root URL configuration mapping all API endpoints, admin views,
webhook handlers, and static/media routes. Organized by domain
with versioned API prefixes.
"""

from django.contrib import admin
from django.urls import include, path, re_path
from django.views.decorators.csrf import csrf_exempt

from . import views
from . import api_views
from . import admin_views
from . import webhook_views


# ─── User Management ────────────────────────────────────────────────

user_patterns = [
    path("", views.user_list, name="user-list"),
    path("<int:pk>/", views.user_detail, name="user-detail"),
    path("<int:pk>/update/", views.user_update, name="user-update"),
    path("<int:pk>/delete/", views.user_delete, name="user-delete"),
    path("register/", views.user_register, name="user-register"),
    path("login/", views.user_login, name="user-login"),
    path("logout/", views.user_logout, name="user-logout"),
    path("password-reset/", views.password_reset_request, name="password-reset"),
    path("password-reset/<str:token>/", views.password_reset_confirm, name="password-reset-confirm"),
    path("verify-email/<str:token>/", views.verify_email, name="verify-email"),
    path("me/", views.current_user_profile, name="current-user"),
    path("me/preferences/", views.user_preferences, name="user-preferences"),
]

# ─── Product Catalog ────────────────────────────────────────────────

product_patterns = [
    path("", views.product_list, name="product-list"),
    path("<int:pk>/", views.product_detail, name="product-detail"),
    path("<int:pk>/reviews/", views.product_reviews, name="product-reviews"),
    path("<int:pk>/reviews/create/", views.create_review, name="create-review"),
    path("categories/", views.category_list, name="category-list"),
    path("categories/<slug:slug>/", views.category_detail, name="category-detail"),
    path("search/", views.product_search, name="product-search"),
    path("featured/", views.featured_products, name="featured-products"),
    re_path(r"^sku/(?P<sku>[A-Z0-9\-]+)/$", views.product_by_sku, name="product-by-sku"),
]

# ─── Orders ─────────────────────────────────────────────────────────

order_patterns = [
    path("", views.order_list, name="order-list"),
    path("create/", views.order_create, name="order-create"),
    path("<int:pk>/", views.order_detail, name="order-detail"),
    path("<int:pk>/cancel/", views.order_cancel, name="order-cancel"),
    path("<int:pk>/status/", views.order_status, name="order-status"),
    path("<int:pk>/tracking/", views.order_tracking, name="order-tracking"),
    path("<int:pk>/invoice/", views.order_invoice, name="order-invoice"),
    path("history/", views.order_history, name="order-history"),
]

# ─── Payments ───────────────────────────────────────────────────────

payment_patterns = [
    path("", views.payment_list, name="payment-list"),
    path("create/", views.payment_create, name="payment-create"),
    path("<int:pk>/", views.payment_detail, name="payment-detail"),
    path("<int:pk>/refund/", views.payment_refund, name="payment-refund"),
    path("<int:pk>/dispute/", views.payment_dispute, name="payment-dispute"),
    path("<int:pk>/receipt/", views.payment_receipt, name="payment-receipt"),
    path("methods/", views.payment_methods_list, name="payment-methods"),
    path("methods/add/", views.payment_method_add, name="payment-method-add"),
    path("methods/<int:pk>/delete/", views.payment_method_delete, name="payment-method-delete"),
]

# ─── Cart & Checkout ────────────────────────────────────────────────

cart_patterns = [
    path("", views.cart_view, name="cart-view"),
    path("add/", views.cart_add_item, name="cart-add"),
    path("remove/<int:item_id>/", views.cart_remove_item, name="cart-remove"),
    path("update/<int:item_id>/", views.cart_update_quantity, name="cart-update"),
    path("clear/", views.cart_clear, name="cart-clear"),
    path("checkout/", views.checkout, name="checkout"),
    path("checkout/confirm/", views.checkout_confirm, name="checkout-confirm"),
]

# ─── Admin Dashboard ────────────────────────────────────────────────

admin_api_patterns = [
    path("dashboard/", admin_views.admin_dashboard, name="admin-dashboard"),
    path("dashboard/stats/", admin_views.admin_stats, name="admin-stats"),
    path("users/", admin_views.admin_user_list, name="admin-users"),
    path("users/<int:pk>/", admin_views.admin_user_detail, name="admin-user-detail"),
    path("users/<int:pk>/ban/", admin_views.admin_ban_user, name="admin-ban-user"),
    path("orders/", admin_views.admin_order_list, name="admin-orders"),
    path("orders/<int:pk>/", admin_views.admin_order_detail, name="admin-order-detail"),
    path("orders/<int:pk>/fulfill/", admin_views.admin_fulfill_order, name="admin-fulfill"),
    path("payments/", admin_views.admin_payment_list, name="admin-payments"),
    path("payments/<int:pk>/refund/", admin_views.admin_refund_payment, name="admin-refund"),
    path("reports/revenue/", admin_views.revenue_report, name="admin-revenue"),
    path("reports/customers/", admin_views.customer_report, name="admin-customers"),
    path("inventory/", admin_views.inventory_management, name="admin-inventory"),
    path("inventory/<int:pk>/restock/", admin_views.restock_product, name="admin-restock"),
]

# ─── Webhooks ───────────────────────────────────────────────────────

webhook_patterns = [
    path("stripe/", csrf_exempt(webhook_views.stripe_webhook), name="webhook-stripe"),
    path("paypal/", csrf_exempt(webhook_views.paypal_webhook), name="webhook-paypal"),
    path("shipping/", csrf_exempt(webhook_views.shipping_webhook), name="webhook-shipping"),
    path("inventory/", csrf_exempt(webhook_views.inventory_webhook), name="webhook-inventory"),
]

# ─── API v1 (REST) ──────────────────────────────────────────────────

api_v1_patterns = [
    path("users/", include(user_patterns)),
    path("products/", include(product_patterns)),
    path("orders/", include(order_patterns)),
    path("payments/", include(payment_patterns)),
    path("cart/", include(cart_patterns)),
]

# ─── API v2 (newer endpoints) ───────────────────────────────────────

api_v2_patterns = [
    path("users/", api_views.user_list_v2, name="v2-user-list"),
    path("users/<int:pk>/", api_views.user_detail_v2, name="v2-user-detail"),
    path("products/", api_views.product_list_v2, name="v2-product-list"),
    path("products/<int:pk>/", api_views.product_detail_v2, name="v2-product-detail"),
    path("orders/", api_views.order_list_v2, name="v2-order-list"),
    path("search/", api_views.search_v2, name="v2-search"),
]

# ─── Misc & Health ──────────────────────────────────────────────────

misc_patterns = [
    path("health/", views.health_check, name="health"),
    path("health/ready/", views.readiness_check, name="readiness"),
    path("health/live/", views.liveness_check, name="liveness"),
    path("config/", views.public_config, name="public-config"),
    re_path(r"^status/(?P<service>[\w\-]+)/$", views.service_status, name="service-status"),
]

# ─── Root URL Configuration ─────────────────────────────────────────

urlpatterns = [
    path("admin/", admin.site.urls),
    path("api/v1/", include(api_v1_patterns)),
    path("api/v2/", include(api_v2_patterns)),
    path("api/admin/", include(admin_api_patterns)),
    path("webhooks/", include(webhook_patterns)),
    path("", include(misc_patterns)),
    re_path(
        r"^api/v1/products/(?P<category_slug>[\w\-]+)/(?P<product_slug>[\w\-]+)/$",
        views.product_by_slugs,
        name="product-by-slugs",
    ),
    re_path(
        r"^api/v1/users/(?P<username>[\w\.]+)/orders/$",
        views.user_orders_by_username,
        name="user-orders-by-username",
    ),
]
