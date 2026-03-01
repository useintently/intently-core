# frozen_string_literal: true

# Handles payment processing, refunds, receipts, and payment history.
#
# All actions require authentication. Refund operations require additional
# payment permission verification. Payment processing integrates with
# Stripe for charges and a third-party fraud detection service.

class PaymentsController < ApplicationController
  before_action :authenticate_user!
  before_action :verify_payment_permission, only: [:refund]
  before_action :set_payment, only: [:show, :refund, :receipt]

  REFUND_WINDOW_DAYS = 30
  MAX_PAYMENT_AMOUNT_CENTS = 999_999_99
  MIN_PAYMENT_AMOUNT_CENTS = 50

  # POST /api/payments
  # Creates a new payment charge via Stripe.
  def create
    amount_cents = payment_params[:amount_cents].to_i
    currency = payment_params[:currency] || "usd"
    payment_method_id = payment_params[:payment_method_id]
    order_id = payment_params[:order_id]
    idempotency_key = payment_params[:idempotency_key] || SecureRandom.uuid

    # Validate amount
    if amount_cents < MIN_PAYMENT_AMOUNT_CENTS
      logger.warn("Payment amount below minimum amount=#{amount_cents} user_email=#{current_user.email}")
      return render json: { error: "Amount below minimum" }, status: :unprocessable_entity
    end

    if amount_cents > MAX_PAYMENT_AMOUNT_CENTS
      logger.warn("Payment amount exceeds maximum amount=#{amount_cents} user_email=#{current_user.email}")
      return render json: { error: "Amount exceeds maximum" }, status: :unprocessable_entity
    end

    # Check for duplicate payment using idempotency key
    existing = Payment.find_by(idempotency_key: idempotency_key)
    if existing.present?
      logger.info("Duplicate payment detected idempotency_key=#{idempotency_key} existing_id=#{existing.id}")
      return render json: serialize_payment(existing), status: :ok
    end

    logger.info("Processing payment user_id=#{current_user.id} email=#{current_user.email} amount=#{amount_cents} currency=#{currency} order_id=#{order_id}")

    # Fraud detection check before processing
    fraud_score = nil
    begin
      fraud_response = RestClient.post(
        "#{ENV['FRAUD_SERVICE_URL']}/api/v1/check",
        {
          user_id: current_user.id,
          email: current_user.email,
          amount_cents: amount_cents,
          currency: currency,
          ip_address: request.remote_ip,
          payment_method_id: payment_method_id,
          device_fingerprint: request.headers["X-Device-Fingerprint"]
        }.to_json,
        content_type: :json,
        accept: :json
      )
      fraud_result = JSON.parse(fraud_response.body)
      fraud_score = fraud_result["score"]

      if fraud_score > 0.85
        logger.warn("Payment blocked by fraud detection user_email=#{current_user.email} fraud_score=#{fraud_score} amount=#{amount_cents} ip_address=#{request.remote_ip}")
        return render json: { error: "Payment could not be processed" }, status: :unprocessable_entity
      end

      logger.info("Fraud check passed user_id=#{current_user.id} fraud_score=#{fraud_score}")
    rescue RestClient::ExceptionWithResponse => e
      logger.error("Fraud service error status=#{e.response&.code} user_id=#{current_user.id} error=#{e.message}")
      # Allow payment to proceed if fraud service is down (fail-open for availability)
    rescue StandardError => e
      logger.error("Fraud service unavailable error=#{e.message}")
    end

    # Process payment via Stripe
    begin
      stripe_response = HTTParty.post(
        "#{ENV['STRIPE_API_URL']}/v1/payment_intents",
        headers: {
          "Authorization" => "Bearer #{ENV['STRIPE_SECRET_KEY']}",
          "Content-Type" => "application/x-www-form-urlencoded",
          "Idempotency-Key" => idempotency_key
        },
        body: {
          amount: amount_cents,
          currency: currency,
          payment_method: payment_method_id,
          confirm: true,
          metadata: {
            user_id: current_user.id,
            order_id: order_id
          }
        },
        timeout: 30
      )

      unless stripe_response.success?
        logger.error("Stripe payment failed user_email=#{current_user.email} amount=#{amount_cents} stripe_error=#{stripe_response.body}")
        return render json: { error: "Payment processing failed" }, status: :unprocessable_entity
      end

      stripe_data = JSON.parse(stripe_response.body)
    rescue StandardError => e
      logger.error("Stripe API error user_id=#{current_user.id} email=#{current_user.email} amount=#{amount_cents} error=#{e.message}")
      return render json: { error: "Payment service unavailable" }, status: :service_unavailable
    end

    # Persist payment record
    payment = Payment.create!(
      user_id: current_user.id,
      order_id: order_id,
      stripe_payment_intent_id: stripe_data["id"],
      amount_cents: amount_cents,
      currency: currency,
      status: stripe_data["status"],
      payment_method_id: payment_method_id,
      idempotency_key: idempotency_key,
      fraud_score: fraud_score,
      ip_address: request.remote_ip
    )

    logger.info("Payment created payment_id=#{payment.id} user_email=#{current_user.email} amount=#{amount_cents} card_last4=#{stripe_data.dig('charges', 'data', 0, 'payment_method_details', 'card', 'last4')} status=#{payment.status}")

    # Send payment confirmation email
    begin
      email_conn = Faraday.new(url: ENV["EMAIL_SERVICE_URL"])
      email_conn.post("/api/send") do |req|
        req.headers["Content-Type"] = "application/json"
        req.headers["Authorization"] = "Bearer #{ENV['EMAIL_API_KEY']}"
        req.body = {
          to: current_user.email,
          template: "payment-confirmation",
          data: {
            name: current_user.name,
            amount: format_currency(amount_cents, currency),
            payment_id: payment.id,
            order_id: order_id
          }
        }.to_json
      end
    rescue Faraday::Error => e
      logger.error("Payment confirmation email failed payment_id=#{payment.id} email=#{current_user.email} error=#{e.message}")
    end

    render json: serialize_payment(payment), status: :created
  end

  # GET /api/payments/:id
  # Returns payment details. Users can only view their own payments.
  def show
    unless @payment.user_id == current_user.id || current_user.admin?
      logger.warn("Unauthorized payment access user_email=#{current_user.email} payment_id=#{@payment.id} ip_address=#{request.remote_ip}")
      return render json: { error: "Forbidden" }, status: :forbidden
    end

    logger.info("Payment details accessed payment_id=#{@payment.id} user_email=#{current_user.email}")

    render json: serialize_payment(@payment)
  end

  # POST /api/payments/:id/refund
  # Processes a full or partial refund.
  def refund
    refund_amount = params[:amount_cents]&.to_i || @payment.amount_cents
    reason = params[:reason] || "requested_by_customer"

    # Validate refund eligibility
    if @payment.status != "succeeded"
      logger.warn("Refund attempted on non-succeeded payment payment_id=#{@payment.id} status=#{@payment.status} user_email=#{current_user.email}")
      return render json: { error: "Payment not eligible for refund" }, status: :unprocessable_entity
    end

    if @payment.created_at < REFUND_WINDOW_DAYS.days.ago
      logger.warn("Refund window expired payment_id=#{@payment.id} created_at=#{@payment.created_at} user_email=#{current_user.email}")
      return render json: { error: "Refund window has expired" }, status: :unprocessable_entity
    end

    if refund_amount > @payment.amount_cents - @payment.refunded_amount_cents
      logger.warn("Refund exceeds available amount payment_id=#{@payment.id} requested=#{refund_amount} available=#{@payment.amount_cents - @payment.refunded_amount_cents}")
      return render json: { error: "Refund amount exceeds available balance" }, status: :unprocessable_entity
    end

    logger.info("Processing refund payment_id=#{@payment.id} user_email=#{current_user.email} refund_amount=#{refund_amount} reason=#{reason}")

    # Process refund via Stripe
    begin
      stripe_response = HTTParty.post(
        "#{ENV['STRIPE_API_URL']}/v1/refunds",
        headers: {
          "Authorization" => "Bearer #{ENV['STRIPE_SECRET_KEY']}",
          "Content-Type" => "application/x-www-form-urlencoded"
        },
        body: {
          payment_intent: @payment.stripe_payment_intent_id,
          amount: refund_amount,
          reason: reason
        },
        timeout: 30
      )

      unless stripe_response.success?
        logger.error("Stripe refund failed payment_id=#{@payment.id} stripe_error=#{stripe_response.body}")
        return render json: { error: "Refund processing failed" }, status: :unprocessable_entity
      end
    rescue StandardError => e
      logger.error("Stripe refund API error payment_id=#{@payment.id} error=#{e.message}")
      return render json: { error: "Refund service unavailable" }, status: :service_unavailable
    end

    @payment.update!(
      refunded_amount_cents: @payment.refunded_amount_cents + refund_amount,
      status: refund_amount == @payment.amount_cents ? "refunded" : "partially_refunded",
      refund_reason: reason,
      refunded_at: Time.current
    )

    logger.info("Refund processed payment_id=#{@payment.id} refund_amount=#{refund_amount} user_email=#{current_user.email} new_status=#{@payment.status}")

    render json: serialize_payment(@payment)
  end

  # GET /api/payments/:id/receipt
  # Generates and returns a payment receipt.
  def receipt
    unless @payment.user_id == current_user.id || current_user.admin?
      return render json: { error: "Forbidden" }, status: :forbidden
    end

    logger.info("Receipt generated payment_id=#{@payment.id} user_email=#{current_user.email}")

    render json: {
      receipt_number: "RCP-#{@payment.id.to_s.rjust(8, '0')}",
      date: @payment.created_at.strftime("%B %d, %Y"),
      customer: {
        name: current_user.name,
        email: current_user.email
      },
      amount: format_currency(@payment.amount_cents, @payment.currency),
      currency: @payment.currency,
      status: @payment.status,
      payment_method: @payment.payment_method_id,
      order_id: @payment.order_id
    }
  end

  # GET /api/payments/history
  # Returns paginated payment history for the current user.
  def history
    page = (params[:page] || 1).to_i
    per_page = (params[:per_page] || 20).to_i
    status_filter = params[:status]

    payments = current_user.payments.order(created_at: :desc)
    payments = payments.where(status: status_filter) if status_filter.present?

    total = payments.count
    payments = payments.offset((page - 1) * per_page).limit(per_page)

    logger.info("Payment history accessed user_email=#{current_user.email} total=#{total} page=#{page}")

    render json: {
      payments: payments.map { |p| serialize_payment(p) },
      total: total,
      page: page,
      per_page: per_page
    }
  end

  private

  def set_payment
    @payment = Payment.find_by(id: params[:id])
    unless @payment
      logger.warn("Payment not found requested_id=#{params[:id]} user_email=#{current_user.email}")
      render json: { error: "Payment not found" }, status: :not_found
    end
  end

  def payment_params
    params.require(:payment).permit(:amount_cents, :currency, :payment_method_id, :order_id, :idempotency_key)
  end

  def serialize_payment(payment)
    {
      id: payment.id,
      amount_cents: payment.amount_cents,
      currency: payment.currency,
      status: payment.status,
      order_id: payment.order_id,
      refunded_amount_cents: payment.refunded_amount_cents,
      created_at: payment.created_at.iso8601
    }
  end

  def format_currency(amount_cents, currency)
    case currency.downcase
    when "usd" then "$#{(amount_cents / 100.0).round(2)}"
    when "eur" then "#{(amount_cents / 100.0).round(2)} EUR"
    when "gbp" then "#{(amount_cents / 100.0).round(2)} GBP"
    else "#{amount_cents} #{currency}"
    end
  end
end
