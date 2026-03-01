# frozen_string_literal: true

# Manages the product catalog: listing, search, CRUD, and related products.
#
# Public actions: index, show, search, featured, related, reviews.
# Protected actions: create, update, destroy (admin only).
# Integrates with an external search index for full-text search capabilities.

class ProductsController < ApplicationController
  before_action :authenticate_user!, only: [:create, :update, :destroy]
  before_action :require_admin, only: [:create, :update, :destroy]
  before_action :set_product, only: [:show, :update, :destroy, :reviews, :related, :inventory]

  SEARCH_RESULTS_LIMIT = 50
  FEATURED_PRODUCTS_LIMIT = 12
  RELATED_PRODUCTS_LIMIT = 8

  # GET /api/products
  # Returns paginated product listing with optional category filter.
  def index
    page = (params[:page] || 1).to_i
    per_page = (params[:per_page] || 24).to_i
    category_id = params[:category_id]
    sort_by = params[:sort] || "created_at"
    sort_order = params[:order] || "desc"

    products = Product.active.includes(:category, :images)

    if category_id.present?
      products = products.where(category_id: category_id)
    end

    if params[:min_price].present?
      products = products.where("price_cents >= ?", (params[:min_price].to_f * 100).to_i)
    end

    if params[:max_price].present?
      products = products.where("price_cents <= ?", (params[:max_price].to_f * 100).to_i)
    end

    allowed_sorts = %w[created_at price_cents name average_rating]
    sort_by = "created_at" unless allowed_sorts.include?(sort_by)
    sort_order = "desc" unless %w[asc desc].include?(sort_order)

    total = products.count
    products = products.order("#{sort_by} #{sort_order}")
                       .offset((page - 1) * per_page)
                       .limit(per_page)

    logger.info("Products listed page=#{page} total=#{total} category=#{category_id}")

    render json: {
      products: products.map { |p| serialize_product_summary(p) },
      total: total,
      page: page,
      per_page: per_page
    }
  end

  # GET /api/products/:id
  # Returns detailed product information including images and reviews summary.
  def show
    view_count = Rails.cache.increment("product_views:#{@product.id}")

    logger.info("Product viewed product_id=#{@product.id} name=#{@product.name} view_count=#{view_count}")

    render json: serialize_product_detail(@product)
  end

  # GET /api/products/search
  # Full-text search via external search index with fallback to database.
  def search
    query = params[:q]

    if query.blank?
      return render json: { error: "Search query is required" }, status: :bad_request
    end

    logger.info("Product search query=#{query} user_agent=#{request.user_agent}")

    # Try external search index first
    begin
      search_conn = Faraday.new(url: ENV["SEARCH_SERVICE_URL"]) do |f|
        f.request :json
        f.response :json
        f.options.timeout = 5
        f.options.open_timeout = 2
      end

      search_response = search_conn.get("/api/index/products/search") do |req|
        req.headers["Authorization"] = "Bearer #{ENV['SEARCH_API_KEY']}"
        req.params["q"] = query
        req.params["limit"] = SEARCH_RESULTS_LIMIT
        req.params["category_id"] = params[:category_id] if params[:category_id].present?
      end

      if search_response.success?
        results = search_response.body
        product_ids = results["hits"].map { |h| h["id"] }
        products = Product.active.where(id: product_ids).includes(:category, :images)

        # Preserve search ranking
        products_by_id = products.index_by(&:id)
        ordered = product_ids.filter_map { |id| products_by_id[id] }

        logger.info("Search completed via index query=#{query} results=#{ordered.size}")

        return render json: {
          products: ordered.map { |p| serialize_product_summary(p) },
          total: results["total_hits"],
          source: "search_index"
        }
      end
    rescue Faraday::Error => e
      logger.warn("Search index unavailable, falling back to database query=#{query} error=#{e.message}")
    end

    # Fallback: database search
    products = Product.active
                      .where("name ILIKE ? OR description ILIKE ?", "%#{query}%", "%#{query}%")
                      .includes(:category, :images)
                      .limit(SEARCH_RESULTS_LIMIT)

    logger.info("Search completed via database fallback query=#{query} results=#{products.size}")

    render json: {
      products: products.map { |p| serialize_product_summary(p) },
      total: products.size,
      source: "database"
    }
  end

  # GET /api/products/featured
  # Returns featured products for the homepage.
  def featured
    products = Product.active
                      .where(featured: true)
                      .includes(:category, :images)
                      .order(average_rating: :desc)
                      .limit(FEATURED_PRODUCTS_LIMIT)

    logger.info("Featured products requested count=#{products.size}")

    render json: { products: products.map { |p| serialize_product_summary(p) } }
  end

  # POST /api/products
  # Creates a new product. Admin only.
  def create
    product = Product.new(product_params)

    unless product.save
      logger.error("Product creation failed errors=#{product.errors.full_messages.join(', ')}")
      return render json: { error: "Validation failed", details: product.errors.full_messages }, status: :unprocessable_entity
    end

    logger.info("Product created product_id=#{product.id} name=#{product.name} price=#{product.price_cents} admin_email=#{current_user.email}")

    # Index in search service
    begin
      Faraday.post("#{ENV['SEARCH_SERVICE_URL']}/api/index/products") do |req|
        req.headers["Content-Type"] = "application/json"
        req.headers["Authorization"] = "Bearer #{ENV['SEARCH_API_KEY']}"
        req.body = {
          id: product.id,
          name: product.name,
          description: product.description,
          category: product.category&.name,
          price_cents: product.price_cents,
          sku: product.sku
        }.to_json
      end
    rescue Faraday::Error => e
      logger.warn("Search indexing failed product_id=#{product.id} error=#{e.message}")
    end

    render json: serialize_product_detail(product), status: :created
  end

  # PUT /api/products/:id
  # Updates an existing product. Admin only.
  def update
    old_price = @product.price_cents

    unless @product.update(product_params)
      logger.error("Product update failed product_id=#{@product.id} errors=#{@product.errors.full_messages.join(', ')}")
      return render json: { error: "Validation failed", details: @product.errors.full_messages }, status: :unprocessable_entity
    end

    logger.info("Product updated product_id=#{@product.id} name=#{@product.name} admin_email=#{current_user.email}")

    # Notify watchers if price dropped
    if @product.price_cents < old_price
      price_drop_pct = ((old_price - @product.price_cents).to_f / old_price * 100).round(1)
      logger.info("Price drop detected product_id=#{@product.id} old_price=#{old_price} new_price=#{@product.price_cents} drop=#{price_drop_pct}%")

      notify_price_drop_watchers(@product, old_price)
    end

    # Update search index
    begin
      Faraday.put("#{ENV['SEARCH_SERVICE_URL']}/api/index/products/#{@product.id}") do |req|
        req.headers["Content-Type"] = "application/json"
        req.headers["Authorization"] = "Bearer #{ENV['SEARCH_API_KEY']}"
        req.body = {
          name: @product.name,
          description: @product.description,
          category: @product.category&.name,
          price_cents: @product.price_cents,
          sku: @product.sku
        }.to_json
      end
    rescue Faraday::Error => e
      logger.warn("Search index update failed product_id=#{@product.id} error=#{e.message}")
    end

    render json: serialize_product_detail(@product)
  end

  # DELETE /api/products/:id
  # Soft-deletes a product. Admin only.
  def destroy
    @product.update!(active: false, deactivated_at: Time.current)

    logger.info("Product deactivated product_id=#{@product.id} name=#{@product.name} admin_email=#{current_user.email}")

    # Remove from search index
    begin
      search_conn = Faraday.new(url: ENV["SEARCH_SERVICE_URL"])
      search_conn.delete("/api/index/products/#{@product.id}") do |req|
        req.headers["Authorization"] = "Bearer #{ENV['SEARCH_API_KEY']}"
      end
    rescue Faraday::Error => e
      logger.warn("Search index removal failed product_id=#{@product.id} error=#{e.message}")
    end

    render json: { message: "Product deactivated" }
  end

  # GET /api/products/:id/reviews
  # Returns paginated reviews for a product.
  def reviews
    page = (params[:page] || 1).to_i
    per_page = (params[:per_page] || 10).to_i

    reviews = @product.reviews
                      .includes(:user)
                      .order(created_at: :desc)
                      .offset((page - 1) * per_page)
                      .limit(per_page)

    total = @product.reviews.count

    render json: {
      reviews: reviews.map { |r| serialize_review(r) },
      total: total,
      average_rating: @product.average_rating,
      page: page
    }
  end

  # GET /api/products/:id/related
  # Returns related products based on category and tags.
  def related
    related = Product.active
                     .where(category_id: @product.category_id)
                     .where.not(id: @product.id)
                     .order("RANDOM()")
                     .limit(RELATED_PRODUCTS_LIMIT)
                     .includes(:images)

    render json: { products: related.map { |p| serialize_product_summary(p) } }
  end

  # GET /api/products/:id/inventory
  # Returns current inventory status. Auth required.
  def inventory
    render json: {
      product_id: @product.id,
      sku: @product.sku,
      stock_count: @product.stock_count,
      reserved: @product.reserved_count,
      available: @product.stock_count - @product.reserved_count,
      low_stock: @product.stock_count < @product.low_stock_threshold,
      updated_at: @product.inventory_updated_at&.iso8601
    }
  end

  private

  def set_product
    @product = Product.find_by(id: params[:id])
    unless @product
      render json: { error: "Product not found" }, status: :not_found
    end
  end

  def product_params
    params.require(:product).permit(
      :name, :description, :price_cents, :currency, :sku,
      :category_id, :stock_count, :low_stock_threshold,
      :featured, :weight_grams, :dimensions,
      tags: [], image_urls: []
    )
  end

  def serialize_product_summary(product)
    {
      id: product.id,
      name: product.name,
      price_cents: product.price_cents,
      currency: product.currency || "usd",
      category: product.category&.name,
      average_rating: product.average_rating,
      review_count: product.reviews_count || 0,
      image_url: product.images.first&.url,
      in_stock: product.stock_count.positive?
    }
  end

  def serialize_product_detail(product)
    {
      id: product.id,
      name: product.name,
      description: product.description,
      price_cents: product.price_cents,
      currency: product.currency || "usd",
      sku: product.sku,
      category: product.category&.as_json(only: [:id, :name]),
      average_rating: product.average_rating,
      review_count: product.reviews_count || 0,
      images: product.images.map { |img| { id: img.id, url: img.url, alt: img.alt_text } },
      stock_count: product.stock_count,
      in_stock: product.stock_count.positive?,
      featured: product.featured,
      tags: product.tags,
      weight_grams: product.weight_grams,
      created_at: product.created_at.iso8601,
      updated_at: product.updated_at.iso8601
    }
  end

  def serialize_review(review)
    {
      id: review.id,
      rating: review.rating,
      title: review.title,
      body: review.body,
      user: {
        id: review.user.id,
        name: review.user.name
      },
      verified_purchase: review.verified_purchase,
      created_at: review.created_at.iso8601
    }
  end

  def notify_price_drop_watchers(product, old_price)
    watchers = ProductWatch.where(product_id: product.id, notify_price_drop: true)
                           .includes(:user)

    watchers.each do |watch|
      begin
        Faraday.post("#{ENV['EMAIL_SERVICE_URL']}/api/send") do |req|
          req.headers["Content-Type"] = "application/json"
          req.headers["Authorization"] = "Bearer #{ENV['EMAIL_API_KEY']}"
          req.body = {
            to: watch.user.email,
            template: "price-drop",
            data: {
              name: watch.user.name,
              product_name: product.name,
              old_price: old_price,
              new_price: product.price_cents
            }
          }.to_json
        end
      rescue Faraday::Error => e
        logger.error("Price drop notification failed user_email=#{watch.user.email} product_id=#{product.id} error=#{e.message}")
      end
    end
  end
end
