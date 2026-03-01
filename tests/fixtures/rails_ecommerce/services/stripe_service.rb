# frozen_string_literal: true

# Encapsulates all Stripe payment gateway interactions.
#
# Provides a unified interface for charges, refunds, customer management,
# balance inquiries, and webhook verification. All external calls are
# instrumented with logging and error handling.
#
# Usage:
#   service = StripeService.new
#   result = service.create_charge(amount_cents: 5000, currency: "usd", ...)
#   refund = service.create_refund(payment_intent_id: "pi_...", amount_cents: 2500)

class StripeService
  BASE_URL = ENV.fetch("STRIPE_API_URL", "https://api.stripe.com")
  DEFAULT_TIMEOUT = 30
  MAX_RETRIES = 3

  def initialize
    @api_key = ENV.fetch("STRIPE_SECRET_KEY")
    @webhook_secret = ENV.fetch("STRIPE_WEBHOOK_SECRET")
    @faraday_conn = build_faraday_connection
  end

  # Creates a payment intent and confirms it immediately.
  #
  # Returns the Stripe PaymentIntent object as a hash, or raises on failure.
  def create_charge(amount_cents:, currency:, payment_method_id:, customer_id: nil, metadata: {})
    logger.info("Creating Stripe charge amount=#{amount_cents} currency=#{currency} customer_id=#{customer_id}")

    response = HTTParty.post(
      "#{BASE_URL}/v1/payment_intents",
      headers: auth_headers.merge("Idempotency-Key" => SecureRandom.uuid),
      body: {
        amount: amount_cents,
        currency: currency,
        payment_method: payment_method_id,
        customer: customer_id,
        confirm: true,
        automatic_payment_methods: { enabled: true, allow_redirects: "never" },
        metadata: metadata
      },
      timeout: DEFAULT_TIMEOUT
    )

    handle_stripe_response(response, "create_charge")
  end

  # Creates a refund for a previously successful payment.
  #
  # Supports full and partial refunds. Returns the Stripe Refund object.
  def create_refund(payment_intent_id:, amount_cents: nil, reason: "requested_by_customer")
    logger.info("Creating Stripe refund payment_intent=#{payment_intent_id} amount=#{amount_cents} reason=#{reason}")

    body = {
      payment_intent: payment_intent_id,
      reason: reason
    }
    body[:amount] = amount_cents if amount_cents.present?

    response = HTTParty.post(
      "#{BASE_URL}/v1/refunds",
      headers: auth_headers,
      body: body,
      timeout: DEFAULT_TIMEOUT
    )

    result = handle_stripe_response(response, "create_refund")

    logger.info("Stripe refund created refund_id=#{result['id']} payment_intent=#{payment_intent_id} amount=#{result['amount']} status=#{result['status']}")

    result
  end

  # Retrieves the current Stripe account balance.
  def get_balance
    logger.info("Fetching Stripe account balance")

    response = @faraday_conn.get("/v1/balance") do |req|
      req.headers["Authorization"] = "Bearer #{@api_key}"
    end

    unless response.success?
      logger.error("Stripe balance fetch failed status=#{response.status} body=#{response.body}")
      raise StripeServiceError, "Failed to fetch balance: #{response.status}"
    end

    balance = JSON.parse(response.body)

    logger.info("Stripe balance retrieved available=#{balance.dig('available', 0, 'amount')} pending=#{balance.dig('pending', 0, 'amount')}")

    balance
  end

  # Creates or updates a Stripe customer record.
  #
  # Maps internal user data to Stripe's customer model.
  def create_customer(email:, name:, phone: nil, metadata: {})
    logger.info("Creating Stripe customer email=#{email} name=#{name}")

    body = {
      email: email,
      name: name,
      metadata: metadata
    }
    body[:phone] = phone if phone.present?

    begin
      response = RestClient.post(
        "#{BASE_URL}/v1/customers",
        body,
        {
          Authorization: "Bearer #{@api_key}",
          content_type: "application/x-www-form-urlencoded"
        }
      )

      customer = JSON.parse(response.body)

      logger.info("Stripe customer created customer_id=#{customer['id']} email=#{email} name=#{name}")

      customer
    rescue RestClient::ExceptionWithResponse => e
      error_body = JSON.parse(e.response.body) rescue { "error" => { "message" => e.message } }
      logger.error("Stripe customer creation failed email=#{email} error=#{error_body.dig('error', 'message')}")
      raise StripeServiceError, "Customer creation failed: #{error_body.dig('error', 'message')}"
    end
  end

  # Updates an existing Stripe customer.
  def update_customer(customer_id:, email: nil, name: nil, phone: nil)
    logger.info("Updating Stripe customer customer_id=#{customer_id} email=#{email} name=#{name}")

    body = {}
    body[:email] = email if email.present?
    body[:name] = name if name.present?
    body[:phone] = phone if phone.present?

    begin
      response = RestClient.post(
        "#{BASE_URL}/v1/customers/#{customer_id}",
        body,
        {
          Authorization: "Bearer #{@api_key}",
          content_type: "application/x-www-form-urlencoded"
        }
      )

      customer = JSON.parse(response.body)

      logger.info("Stripe customer updated customer_id=#{customer_id} email=#{customer['email']}")

      customer
    rescue RestClient::ExceptionWithResponse => e
      logger.error("Stripe customer update failed customer_id=#{customer_id} error=#{e.message}")
      raise StripeServiceError, "Customer update failed: #{e.message}"
    end
  end

  # Retrieves a payment intent by ID.
  def get_payment_intent(payment_intent_id)
    logger.info("Fetching payment intent payment_intent_id=#{payment_intent_id}")

    response = @faraday_conn.get("/v1/payment_intents/#{payment_intent_id}") do |req|
      req.headers["Authorization"] = "Bearer #{@api_key}"
    end

    unless response.success?
      logger.error("Payment intent fetch failed payment_intent_id=#{payment_intent_id} status=#{response.status}")
      raise StripeServiceError, "Failed to fetch payment intent: #{response.status}"
    end

    JSON.parse(response.body)
  end

  # Lists payment methods for a customer.
  def list_payment_methods(customer_id:, type: "card")
    logger.info("Listing payment methods customer_id=#{customer_id} type=#{type}")

    response = @faraday_conn.get("/v1/payment_methods") do |req|
      req.headers["Authorization"] = "Bearer #{@api_key}"
      req.params["customer"] = customer_id
      req.params["type"] = type
    end

    unless response.success?
      logger.error("Payment methods list failed customer_id=#{customer_id} status=#{response.status}")
      raise StripeServiceError, "Failed to list payment methods: #{response.status}"
    end

    data = JSON.parse(response.body)

    logger.info("Payment methods retrieved customer_id=#{customer_id} count=#{data['data'].size}")

    data["data"]
  end

  # Verifies a Stripe webhook signature and parses the event.
  #
  # Uses Net::HTTP directly for webhook verification endpoint.
  def verify_webhook(payload:, signature:)
    logger.info("Verifying Stripe webhook signature")

    # Verify signature locally using the webhook secret
    timestamp, received_sig = parse_stripe_signature(signature)

    expected_sig = OpenSSL::HMAC.hexdigest(
      "SHA256",
      @webhook_secret,
      "#{timestamp}.#{payload}"
    )

    unless ActiveSupport::SecurityUtils.secure_compare(received_sig, expected_sig)
      logger.warn("Invalid webhook signature received ip_address=#{Thread.current[:request_ip]}")
      raise StripeServiceError, "Invalid webhook signature"
    end

    event = JSON.parse(payload)

    logger.info("Webhook verified event_type=#{event['type']} event_id=#{event['id']}")

    event
  end

  # Checks Stripe API health status.
  def health_check
    uri = URI("#{BASE_URL}/v1/balance")
    response = Net::HTTP.get(uri)
    parsed = JSON.parse(response)

    logger.info("Stripe health check passed")

    { status: "healthy", livemode: parsed["livemode"] }
  rescue StandardError => e
    logger.error("Stripe health check failed error=#{e.message}")
    { status: "unhealthy", error: e.message }
  end

  # Retrieves upcoming invoices for a subscription customer.
  def get_upcoming_invoice(customer_id:)
    logger.info("Fetching upcoming invoice customer_id=#{customer_id}")

    uri = URI("#{BASE_URL}/v1/invoices/upcoming?customer=#{customer_id}")
    http = Net::HTTP.new(uri.host, uri.port)
    http.use_ssl = true
    http.open_timeout = 5
    http.read_timeout = DEFAULT_TIMEOUT

    request = Net::HTTP::Get.new(uri)
    request["Authorization"] = "Bearer #{@api_key}"

    response = http.request(request)

    unless response.is_a?(Net::HTTPSuccess)
      logger.error("Upcoming invoice fetch failed customer_id=#{customer_id} status=#{response.code}")
      raise StripeServiceError, "Failed to fetch upcoming invoice: #{response.code}"
    end

    invoice = JSON.parse(response.body)

    logger.info("Upcoming invoice retrieved customer_id=#{customer_id} amount=#{invoice['amount_due']} currency=#{invoice['currency']}")

    invoice
  end

  private

  def build_faraday_connection
    Faraday.new(url: BASE_URL) do |f|
      f.request :url_encoded
      f.response :json, parser_options: { symbolize_names: false }
      f.options.timeout = DEFAULT_TIMEOUT
      f.options.open_timeout = 10
      f.adapter Faraday.default_adapter
    end
  end

  def auth_headers
    {
      "Authorization" => "Bearer #{@api_key}",
      "Content-Type" => "application/x-www-form-urlencoded",
      "Stripe-Version" => "2024-12-18.acacia"
    }
  end

  def handle_stripe_response(response, operation)
    unless response.success?
      error_body = begin
                     JSON.parse(response.body)
                   rescue JSON::ParserError
                     { "error" => { "message" => response.body } }
                   end

      error_message = error_body.dig("error", "message") || "Unknown error"
      error_code = error_body.dig("error", "code")

      logger.error("Stripe #{operation} failed status=#{response.code} error=#{error_message} code=#{error_code}")

      raise StripeServiceError, "#{operation} failed: #{error_message} (#{error_code})"
    end

    JSON.parse(response.body)
  end

  def parse_stripe_signature(header)
    pairs = header.split(",").map { |item| item.strip.split("=", 2) }.to_h
    timestamp = pairs["t"]
    signature = pairs["v1"]

    unless timestamp && signature
      raise StripeServiceError, "Malformed Stripe signature header"
    end

    [timestamp, signature]
  end

  class StripeServiceError < StandardError; end
end
