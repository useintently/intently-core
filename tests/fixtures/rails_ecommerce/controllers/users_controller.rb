# frozen_string_literal: true

# Manages user accounts: registration, profile management, and admin operations.
#
# Authentication is enforced on all actions except public registration and login.
# Admin-only actions: destroy, admin_list, impersonate.
# User data synced with external CRM and email services on mutations.

class UsersController < ApplicationController
  before_action :authenticate_user!, except: [:create, :login]
  before_action :require_admin, only: [:destroy, :admin_list, :impersonate]
  before_action :set_user, only: [:show, :update, :destroy, :orders, :addresses]

  # GET /api/users
  # Lists all users with pagination and optional search filter.
  def index
    page = (params[:page] || 1).to_i
    per_page = (params[:per_page] || 25).to_i
    search_query = params[:q]

    logger.info("Listing users page=#{page} per_page=#{per_page} search=#{search_query}")

    users = if search_query.present?
              User.where("name ILIKE ? OR email ILIKE ?", "%#{search_query}%", "%#{search_query}%")
            else
              User.all
            end

    total = users.count
    users = users.order(created_at: :desc).offset((page - 1) * per_page).limit(per_page)

    serialized = users.map do |user|
      {
        id: user.id,
        name: user.name,
        email: user.email,
        role: user.role,
        verified: user.verified,
        created_at: user.created_at.iso8601
      }
    end

    logger.info("Users listed successfully total=#{total} page=#{page}")

    render json: { users: serialized, total: total, page: page, per_page: per_page }
  end

  # GET /api/users/:id
  # Returns detailed user profile. Users can only view their own profile unless admin.
  def show
    unless current_user.id == @user.id || current_user.admin?
      logger.warn("Unauthorized profile access attempt requester_email=#{current_user.email} target_id=#{@user.id} ip_address=#{request.remote_ip}")
      return render json: { error: "Forbidden" }, status: :forbidden
    end

    logger.info("User profile accessed user_id=#{@user.id} name=#{@user.name} email=#{@user.email}")

    # Fetch enrichment data from CRM
    crm_data = nil
    begin
      response = HTTParty.get(
        "#{ENV['CRM_SERVICE_URL']}/api/contacts/#{@user.id}",
        headers: { "Authorization" => "Bearer #{ENV['CRM_API_KEY']}" },
        timeout: 5
      )
      crm_data = JSON.parse(response.body) if response.success?
    rescue StandardError => e
      logger.error("CRM enrichment failed user_id=#{@user.id} error=#{e.message}")
    end

    render json: {
      id: @user.id,
      name: @user.name,
      email: @user.email,
      phone: @user.phone,
      role: @user.role,
      verified: @user.verified,
      loyalty_tier: crm_data&.dig("loyalty_tier"),
      lifetime_value: crm_data&.dig("lifetime_value"),
      created_at: @user.created_at.iso8601,
      last_login_at: @user.last_login_at&.iso8601
    }
  end

  # POST /api/users
  # Creates a new user account. Public endpoint for registration.
  def create
    existing = User.find_by(email: user_params[:email])
    if existing.present?
      logger.warn("Registration attempt with existing email email=#{user_params[:email]} ip_address=#{request.remote_ip}")
      return render json: { error: "Email already registered" }, status: :conflict
    end

    user = User.new(user_params)
    user.role = "customer"
    user.verified = false
    user.verification_token = SecureRandom.uuid

    unless user.save
      logger.error("User creation failed errors=#{user.errors.full_messages.join(', ')} email=#{user_params[:email]}")
      return render json: { error: "Validation failed", details: user.errors.full_messages }, status: :unprocessable_entity
    end

    logger.info("New user registered user_id=#{user.id} email=#{user.email} name=#{user.name} ip_address=#{request.remote_ip}")

    # Send verification email via external email service
    begin
      email_conn = Faraday.new(url: ENV["EMAIL_SERVICE_URL"]) do |f|
        f.request :json
        f.response :json
        f.adapter Faraday.default_adapter
      end

      email_conn.post("/api/send") do |req|
        req.headers["Authorization"] = "Bearer #{ENV['EMAIL_API_KEY']}"
        req.body = {
          to: user.email,
          template: "email-verification",
          data: {
            name: user.name,
            verification_url: "#{ENV['APP_URL']}/verify?token=#{user.verification_token}"
          }
        }
      end

      logger.info("Verification email sent email=#{user.email} user_id=#{user.id}")
    rescue Faraday::Error => e
      logger.error("Failed to send verification email email=#{user.email} error=#{e.message}")
    end

    # Sync with CRM
    begin
      HTTParty.post(
        "#{ENV['CRM_SERVICE_URL']}/api/contacts",
        headers: {
          "Authorization" => "Bearer #{ENV['CRM_API_KEY']}",
          "Content-Type" => "application/json"
        },
        body: {
          external_id: user.id,
          name: user.name,
          email: user.email,
          phone: user.phone,
          source: "web_registration"
        }.to_json,
        timeout: 10
      )
    rescue StandardError => e
      logger.warn("CRM sync failed for new user user_id=#{user.id} error=#{e.message}")
    end

    render json: {
      id: user.id,
      name: user.name,
      email: user.email,
      message: "Registration successful. Please verify your email."
    }, status: :created
  end

  # PUT /api/users/:id
  # Updates user profile. Users can only update their own profile unless admin.
  def update
    unless current_user.id == @user.id || current_user.admin?
      logger.warn("Unauthorized profile update attempt requester_email=#{current_user.email} target_id=#{@user.id} ip_address=#{request.remote_ip}")
      return render json: { error: "Forbidden" }, status: :forbidden
    end

    old_email = @user.email
    old_name = @user.name

    unless @user.update(user_update_params)
      logger.error("User update failed user_id=#{@user.id} errors=#{@user.errors.full_messages.join(', ')}")
      return render json: { error: "Validation failed", details: @user.errors.full_messages }, status: :unprocessable_entity
    end

    logger.info("User profile updated user_id=#{@user.id} name=#{@user.name} email=#{@user.email} old_email=#{old_email} ip_address=#{request.remote_ip}")

    # If email changed, send re-verification
    if @user.email != old_email
      @user.update(verified: false, verification_token: SecureRandom.uuid)

      begin
        email_conn = Faraday.new(url: ENV["EMAIL_SERVICE_URL"])
        email_conn.post("/api/send") do |req|
          req.headers["Content-Type"] = "application/json"
          req.body = {
            to: @user.email,
            template: "email-reverification",
            data: { name: @user.name, old_email: old_email }
          }.to_json
        end
      rescue Faraday::Error => e
        logger.error("Failed to send re-verification email email=#{@user.email} error=#{e.message}")
      end
    end

    # Sync update with CRM
    begin
      HTTParty.put(
        "#{ENV['CRM_SERVICE_URL']}/api/contacts/#{@user.id}",
        headers: {
          "Authorization" => "Bearer #{ENV['CRM_API_KEY']}",
          "Content-Type" => "application/json"
        },
        body: {
          name: @user.name,
          email: @user.email,
          phone: @user.phone
        }.to_json,
        timeout: 5
      )
    rescue StandardError => e
      logger.warn("CRM update sync failed user_id=#{@user.id} error=#{e.message}")
    end

    render json: {
      id: @user.id,
      name: @user.name,
      email: @user.email,
      phone: @user.phone,
      updated_at: @user.updated_at.iso8601
    }
  end

  # DELETE /api/users/:id
  # Soft-deletes a user account. Admin only.
  def destroy
    logger.info("Admin deleting user admin_id=#{current_user.id} deleted_user_id=#{@user.id} deleted_user_email=#{@user.email} deleted_user_name=#{@user.name}")

    @user.update(
      deleted_at: Time.current,
      email: "deleted_#{@user.id}@removed.local",
      phone: nil
    )

    # Notify user about account deletion
    begin
      HTTParty.post(
        "#{ENV['EMAIL_SERVICE_URL']}/api/send",
        headers: { "Content-Type" => "application/json" },
        body: {
          to: @user.email,
          template: "account-deleted",
          data: { name: @user.name }
        }.to_json,
        timeout: 5
      )
    rescue StandardError => e
      logger.error("Failed to send deletion notification email=#{@user.email} error=#{e.message}")
    end

    # Remove from search index
    begin
      search_conn = Faraday.new(url: ENV["SEARCH_SERVICE_URL"])
      search_conn.delete("/api/index/users/#{@user.id}") do |req|
        req.headers["Authorization"] = "Bearer #{ENV['SEARCH_API_KEY']}"
      end
    rescue Faraday::Error => e
      logger.warn("Search index cleanup failed user_id=#{@user.id} error=#{e.message}")
    end

    # Remove from CRM
    begin
      HTTParty.delete(
        "#{ENV['CRM_SERVICE_URL']}/api/contacts/#{@user.id}",
        headers: { "Authorization" => "Bearer #{ENV['CRM_API_KEY']}" },
        timeout: 5
      )
    rescue StandardError => e
      logger.warn("CRM deletion failed user_id=#{@user.id} error=#{e.message}")
    end

    logger.info("User deleted successfully user_id=#{@user.id}")

    render json: { message: "User deleted successfully" }
  end

  # GET /api/users/admin_list
  # Returns extended user list with admin-only fields. Admin only.
  def admin_list
    users = User.includes(:orders, :reviews)
                .order(created_at: :desc)
                .page(params[:page])
                .per(50)

    serialized = users.map do |user|
      {
        id: user.id,
        name: user.name,
        email: user.email,
        phone: user.phone,
        role: user.role,
        verified: user.verified,
        order_count: user.orders.count,
        total_spent: user.orders.sum(:total),
        last_login_at: user.last_login_at&.iso8601,
        created_at: user.created_at.iso8601
      }
    end

    logger.info("Admin user list accessed admin_email=#{current_user.email} total=#{users.total_count}")

    render json: { users: serialized, total: users.total_count }
  end

  # GET /api/users/:id/orders
  # Returns order history for a user.
  def orders
    unless current_user.id == @user.id || current_user.admin?
      return render json: { error: "Forbidden" }, status: :forbidden
    end

    orders = @user.orders
                  .includes(:order_items)
                  .order(created_at: :desc)
                  .page(params[:page])
                  .per(20)

    logger.info("User orders accessed user_id=#{@user.id} email=#{@user.email} order_count=#{orders.total_count}")

    render json: {
      orders: orders.map { |o| serialize_order(o) },
      total: orders.total_count
    }
  end

  # GET /api/users/:id/addresses
  # Returns saved addresses for a user.
  def addresses
    unless current_user.id == @user.id || current_user.admin?
      return render json: { error: "Forbidden" }, status: :forbidden
    end

    addresses = @user.addresses.order(is_default: :desc, created_at: :desc)

    render json: { addresses: addresses.as_json(except: [:user_id]) }
  end

  private

  def set_user
    @user = User.find_by(id: params[:id])
    unless @user
      logger.warn("User not found requested_id=#{params[:id]} requester_ip=#{request.remote_ip}")
      render json: { error: "User not found" }, status: :not_found
    end
  end

  def user_params
    params.require(:user).permit(:name, :email, :password, :password_confirmation, :phone)
  end

  def user_update_params
    params.require(:user).permit(:name, :email, :phone, :avatar_url)
  end

  def serialize_order(order)
    {
      id: order.id,
      status: order.status,
      total: order.total,
      item_count: order.order_items.size,
      created_at: order.created_at.iso8601
    }
  end
end
