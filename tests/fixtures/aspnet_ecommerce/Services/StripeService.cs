using System;
using System.Collections.Generic;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Threading.Tasks;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;
using EcommerceApi.Configuration;
using EcommerceApi.Models;

namespace EcommerceApi.Services
{
    public class StripeService : IStripeService
    {
        private readonly HttpClient _httpClient;
        private readonly ILogger<StripeService> _logger;
        private readonly StripeSettings _settings;

        public StripeService(
            HttpClient httpClient,
            ILogger<StripeService> logger,
            IOptions<StripeSettings> settings)
        {
            _httpClient = httpClient;
            _logger = logger;
            _settings = settings.Value;

            _httpClient.BaseAddress = new Uri("https://api.stripe.com/");
            _httpClient.DefaultRequestHeaders.Authorization =
                new AuthenticationHeaderValue("Bearer", _settings.SecretKey);
        }

        public async Task<StripeChargeResult> CreateChargeAsync(
            decimal amount,
            string currency,
            string customerEmail,
            string paymentMethodId,
            Dictionary<string, string>? metadata = null)
        {
            _logger.LogInformation(
                "Creating Stripe charge: amount={Amount}, currency={Currency}, email={Email}, paymentMethod={PaymentMethodId}",
                amount, currency, customerEmail, paymentMethodId);

            var payload = new
            {
                amount = (int)(amount * 100),
                currency = currency.ToLowerInvariant(),
                payment_method = paymentMethodId,
                receipt_email = customerEmail,
                confirm = true,
                metadata = metadata ?? new Dictionary<string, string>()
            };

            try
            {
                var response = await _httpClient.PostAsJsonAsync("v1/charges", payload);
                var responseBody = await response.Content.ReadAsStringAsync();

                if (!response.IsSuccessStatusCode)
                {
                    _logger.LogError(
                        "Stripe charge creation failed: status={StatusCode}, email={Email}, amount={Amount}, response={Response}",
                        response.StatusCode, customerEmail, amount, responseBody);

                    return new StripeChargeResult
                    {
                        Success = false,
                        ErrorMessage = $"Stripe returned {response.StatusCode}"
                    };
                }

                var result = JsonSerializer.Deserialize<StripeChargeResponse>(responseBody);
                _logger.LogInformation(
                    "Stripe charge created: chargeId={ChargeId}, amount={Amount}, email={Email}, status={Status}",
                    result.Id, amount, customerEmail, result.Status);

                return new StripeChargeResult
                {
                    Success = true,
                    ChargeId = result.Id,
                    Status = result.Status,
                    ReceiptUrl = result.ReceiptUrl
                };
            }
            catch (HttpRequestException ex)
            {
                _logger.LogError(ex,
                    "Stripe API connection failed during charge creation: email={Email}, amount={Amount}",
                    customerEmail, amount);
                throw new PaymentGatewayException("Failed to connect to Stripe API", ex);
            }
        }

        public async Task<StripeRefundResult> CreateRefundAsync(
            string chargeId,
            decimal? amount = null,
            string? reason = null)
        {
            _logger.LogInformation(
                "Creating Stripe refund: chargeId={ChargeId}, amount={Amount}, reason={Reason}",
                chargeId, amount, reason);

            var payload = new Dictionary<string, object>
            {
                ["charge"] = chargeId
            };

            if (amount.HasValue)
            {
                payload["amount"] = (int)(amount.Value * 100);
            }

            if (!string.IsNullOrEmpty(reason))
            {
                payload["reason"] = reason;
            }

            try
            {
                var response = await _httpClient.PostAsJsonAsync("v1/refunds", payload);
                var responseBody = await response.Content.ReadAsStringAsync();

                if (!response.IsSuccessStatusCode)
                {
                    _logger.LogError(
                        "Stripe refund failed: chargeId={ChargeId}, status={StatusCode}, response={Response}",
                        chargeId, response.StatusCode, responseBody);

                    return new StripeRefundResult
                    {
                        Success = false,
                        ErrorMessage = $"Stripe refund returned {response.StatusCode}"
                    };
                }

                var result = JsonSerializer.Deserialize<StripeRefundResponse>(responseBody);
                _logger.LogInformation(
                    "Stripe refund created: refundId={RefundId}, chargeId={ChargeId}, amount={Amount}, status={Status}",
                    result.Id, chargeId, result.Amount / 100.0m, result.Status);

                return new StripeRefundResult
                {
                    Success = true,
                    RefundId = result.Id,
                    Status = result.Status,
                    Amount = result.Amount / 100.0m
                };
            }
            catch (HttpRequestException ex)
            {
                _logger.LogError(ex, "Stripe API connection failed during refund: chargeId={ChargeId}", chargeId);
                throw new PaymentGatewayException("Failed to connect to Stripe API for refund", ex);
            }
        }

        public async Task<StripeBalanceResult> GetBalanceAsync()
        {
            _logger.LogInformation("Retrieving Stripe account balance");

            try
            {
                var response = await _httpClient.GetAsync("v1/balance");
                var responseBody = await response.Content.ReadAsStringAsync();

                if (!response.IsSuccessStatusCode)
                {
                    _logger.LogError("Stripe balance retrieval failed: status={StatusCode}", response.StatusCode);
                    return new StripeBalanceResult { Success = false };
                }

                var result = JsonSerializer.Deserialize<StripeBalanceResponse>(responseBody);
                _logger.LogInformation("Stripe balance retrieved: available={Available}, pending={Pending}",
                    result.Available?.FirstOrDefault()?.Amount, result.Pending?.FirstOrDefault()?.Amount);

                return new StripeBalanceResult
                {
                    Success = true,
                    Available = result.Available,
                    Pending = result.Pending
                };
            }
            catch (HttpRequestException ex)
            {
                _logger.LogError(ex, "Stripe API connection failed during balance retrieval");
                throw new PaymentGatewayException("Failed to connect to Stripe API for balance", ex);
            }
        }

        public async Task<StripeCustomerResult> CreateCustomerAsync(
            string email,
            string name,
            string? phone = null,
            Dictionary<string, string>? metadata = null)
        {
            _logger.LogInformation(
                "Creating Stripe customer: email={Email}, name={Name}, phone={Phone}",
                email, name, phone);

            var payload = new
            {
                email = email,
                name = name,
                phone = phone,
                metadata = metadata ?? new Dictionary<string, string>()
            };

            try
            {
                var response = await _httpClient.PostAsJsonAsync("v1/customers", payload);
                var responseBody = await response.Content.ReadAsStringAsync();

                if (!response.IsSuccessStatusCode)
                {
                    _logger.LogError(
                        "Stripe customer creation failed: email={Email}, name={Name}, status={StatusCode}",
                        email, name, response.StatusCode);

                    return new StripeCustomerResult
                    {
                        Success = false,
                        ErrorMessage = $"Customer creation failed with status {response.StatusCode}"
                    };
                }

                var result = JsonSerializer.Deserialize<StripeCustomerResponse>(responseBody);
                _logger.LogInformation(
                    "Stripe customer created: customerId={CustomerId}, email={Email}, name={Name}",
                    result.Id, email, name);

                return new StripeCustomerResult
                {
                    Success = true,
                    CustomerId = result.Id,
                    Email = email
                };
            }
            catch (HttpRequestException ex)
            {
                _logger.LogError(ex,
                    "Stripe API connection failed during customer creation: email={Email}, name={Name}",
                    email, name);
                throw new PaymentGatewayException("Failed to connect to Stripe API for customer creation", ex);
            }
        }

        public async Task<bool> ValidateWebhookAsync(string payload, string signatureHeader)
        {
            _logger.LogDebug("Validating Stripe webhook signature");

            if (string.IsNullOrEmpty(signatureHeader))
            {
                _logger.LogWarning("Webhook signature header is missing");
                return false;
            }

            try
            {
                var elements = signatureHeader.Split(',');
                string? timestamp = null;
                string? signature = null;

                foreach (var element in elements)
                {
                    var parts = element.Trim().Split('=', 2);
                    if (parts.Length == 2)
                    {
                        if (parts[0] == "t") timestamp = parts[1];
                        if (parts[0] == "v1") signature = parts[1];
                    }
                }

                if (timestamp == null || signature == null)
                {
                    _logger.LogWarning("Webhook signature header has invalid format");
                    return false;
                }

                var signedPayload = $"{timestamp}.{payload}";
                using var hmac = new HMACSHA256(Encoding.UTF8.GetBytes(_settings.WebhookSecret));
                var computedHash = hmac.ComputeHash(Encoding.UTF8.GetBytes(signedPayload));
                var computedSignature = BitConverter.ToString(computedHash).Replace("-", "").ToLowerInvariant();

                var isValid = CryptographicOperations.FixedTimeEquals(
                    Encoding.UTF8.GetBytes(computedSignature),
                    Encoding.UTF8.GetBytes(signature));

                if (!isValid)
                {
                    _logger.LogWarning("Webhook signature validation failed: timestamp={Timestamp}", timestamp);
                }
                else
                {
                    _logger.LogInformation("Webhook signature validated successfully");
                }

                return isValid;
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error during webhook signature validation");
                return false;
            }
        }
    }
}
