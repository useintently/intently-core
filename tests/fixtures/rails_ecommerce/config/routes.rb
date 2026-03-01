# frozen_string_literal: true

# Ecommerce platform routing configuration.
#
# All API routes are prefixed with /api and versioned.
# Admin routes are namespaced under /admin.
# Health and metrics are public infrastructure endpoints.

Rails.application.routes.draw do
  # ---------------------------------------------------------------------------
  # Infrastructure endpoints (no auth required)
  # ---------------------------------------------------------------------------

  get '/health', to: 'health#show'
  get '/metrics', to: 'metrics#show'
  get '/version', to: 'version#show'

  logger.info("Infrastructure routes loaded")

  # ---------------------------------------------------------------------------
  # Authentication & Registration
  # ---------------------------------------------------------------------------

  post '/api/users/login', to: 'sessions#create'
  post '/api/users/register', to: 'registrations#create'
  post '/api/users/forgot-password', to: 'passwords#create'
  post '/api/users/reset-password', to: 'passwords#update'
  post '/api/users/verify-email', to: 'verifications#create'
  post '/api/users/refresh-token', to: 'sessions#refresh'
  delete '/api/users/logout', to: 'sessions#destroy'

  logger.info("Authentication routes configured")

  # ---------------------------------------------------------------------------
  # User management
  # ---------------------------------------------------------------------------

  get '/api/users', to: 'users#index'
  post '/api/users', to: 'users#create'
  get '/api/users/:id', to: 'users#show'
  put '/api/users/:id', to: 'users#update'
  delete '/api/users/:id', to: 'users#destroy'
  get '/api/users/:id/orders', to: 'users#orders'
  get '/api/users/:id/addresses', to: 'users#addresses'
  post '/api/users/:id/addresses', to: 'users#add_address'
  put '/api/users/:id/addresses/:address_id', to: 'users#update_address'
  delete '/api/users/:id/addresses/:address_id', to: 'users#remove_address'

  # User routes require authentication
  before_action :authenticate_user!

  # ---------------------------------------------------------------------------
  # Payment processing
  # ---------------------------------------------------------------------------

  post '/api/payments', to: 'payments#create'
  get '/api/payments/:id', to: 'payments#show'
  post '/api/payments/:id/refund', to: 'payments#refund'
  get '/api/payments/:id/receipt', to: 'payments#receipt'
  post '/api/payments/webhook', to: 'payments#webhook'
  get '/api/payments/methods', to: 'payment_methods#index'
  post '/api/payments/methods', to: 'payment_methods#create'
  delete '/api/payments/methods/:id', to: 'payment_methods#destroy'

  logger.info("Payment routes configured for Stripe and PayPal integration")

  # ---------------------------------------------------------------------------
  # Product catalog (resources + custom routes)
  # ---------------------------------------------------------------------------

  resources :products
  resources :categories
  resources :reviews

  get '/api/products/search', to: 'products#search'
  get '/api/products/featured', to: 'products#featured'
  get '/api/products/:id/reviews', to: 'products#reviews'
  post '/api/products/:id/reviews', to: 'reviews#create'
  get '/api/products/:id/related', to: 'products#related'
  get '/api/products/:id/inventory', to: 'products#inventory'

  get '/api/categories/:id/products', to: 'categories#products'
  get '/api/categories/tree', to: 'categories#tree'

  # ---------------------------------------------------------------------------
  # Order management
  # ---------------------------------------------------------------------------

  resources :orders

  get '/api/orders/:id/tracking', to: 'orders#tracking'
  post '/api/orders/:id/cancel', to: 'orders#cancel'
  get '/api/orders/:id/invoice', to: 'orders#invoice'

  # ---------------------------------------------------------------------------
  # Shopping cart
  # ---------------------------------------------------------------------------

  get '/api/cart', to: 'cart#show'
  post '/api/cart/items', to: 'cart#add_item'
  put '/api/cart/items/:id', to: 'cart#update_item'
  delete '/api/cart/items/:id', to: 'cart#remove_item'
  post '/api/cart/checkout', to: 'cart#checkout'
  post '/api/cart/apply-coupon', to: 'cart#apply_coupon'
  delete '/api/cart/coupon', to: 'cart#remove_coupon'

  # ---------------------------------------------------------------------------
  # Wishlists
  # ---------------------------------------------------------------------------

  get '/api/wishlists', to: 'wishlists#index'
  post '/api/wishlists', to: 'wishlists#create'
  get '/api/wishlists/:id', to: 'wishlists#show'
  post '/api/wishlists/:id/items', to: 'wishlists#add_item'
  delete '/api/wishlists/:id/items/:item_id', to: 'wishlists#remove_item'

  # ---------------------------------------------------------------------------
  # Notifications
  # ---------------------------------------------------------------------------

  get '/api/notifications', to: 'notifications#index'
  put '/api/notifications/:id/read', to: 'notifications#mark_read'
  post '/api/notifications/read-all', to: 'notifications#mark_all_read'

  # ---------------------------------------------------------------------------
  # Admin namespace — all routes require admin auth
  # ---------------------------------------------------------------------------

  namespace :admin do
    get '/dashboard', to: 'dashboard#show'
    get '/analytics', to: 'analytics#index'
    get '/analytics/revenue', to: 'analytics#revenue'
    get '/analytics/users', to: 'analytics#users'

    resources :products
    resources :orders
    resources :users

    get '/inventory', to: 'inventory#index'
    put '/inventory/:product_id', to: 'inventory#update'

    get '/reports/sales', to: 'reports#sales'
    get '/reports/returns', to: 'reports#returns'
    get '/reports/customers', to: 'reports#customers'

    post '/promotions', to: 'promotions#create'
    put '/promotions/:id', to: 'promotions#update'
    delete '/promotions/:id', to: 'promotions#destroy'
    get '/promotions', to: 'promotions#index'

    post '/bulk/products/import', to: 'bulk#import_products'
    post '/bulk/products/export', to: 'bulk#export_products'
    post '/bulk/orders/export', to: 'bulk#export_orders'

    logger.info("Admin routes loaded")
  end

  # ---------------------------------------------------------------------------
  # Webhooks (external integrations)
  # ---------------------------------------------------------------------------

  post '/webhooks/stripe', to: 'webhooks#stripe'
  post '/webhooks/paypal', to: 'webhooks#paypal'
  post '/webhooks/shipping', to: 'webhooks#shipping'
  post '/webhooks/inventory', to: 'webhooks#inventory'

  # ---------------------------------------------------------------------------
  # API versioning
  # ---------------------------------------------------------------------------

  namespace :api do
    namespace :v2 do
      resources :products
      resources :orders
      get '/search', to: 'search#index'
    end
  end

  logger.info("All application routes loaded successfully")

  # ---------------------------------------------------------------------------
  # Catch-all for unmatched routes
  # ---------------------------------------------------------------------------

  get '*unmatched', to: 'errors#not_found'
end
