using System;
using System.Collections.Generic;
using System.Linq;
using System.Net.Http;
using System.Net.Http.Json;
using System.Security.Claims;
using System.Text;
using System.Text.Json;
using System.Threading.Tasks;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;
using EcommerceApi.Models;
using EcommerceApi.Services;
using EcommerceApi.DTOs;
using EcommerceApi.Configuration;

namespace EcommerceApi.Controllers
{
    [ApiController]
    [Route("api/v1/[controller]")]
    public class PaymentsController : ControllerBase
    {
        private readonly IPaymentRepository _paymentRepository;
        private readonly IOrderRepository _orderRepository;
        private readonly ILogger<PaymentsController> _logger;
        private readonly HttpClient _httpClient;
        private readonly StripeSettings _stripeSettings;

        public PaymentsController(
            IPaymentRepository paymentRepository,
            IOrderRepository orderRepository,
            ILogger<PaymentsController> logger,
            IHttpClientFactory httpClientFactory,
            IOptions<StripeSettings> stripeSettings)
        {
            _paymentRepository = paymentRepository;
            _orderRepository = orderRepository;
            _logger = logger;
            _httpClient = httpClientFactory.CreateClient("PaymentGateway");
            _stripeSettings = stripeSettings.Value;
        }

        [HttpPost]
        [Authorize]
        public async Task<ActionResult<PaymentDto>> CreatePayment([FromBody] CreatePaymentRequest request)
        {
            var userId = User.FindFirst(ClaimTypes.NameIdentifier)?.Value;
            _logger.LogInformation(
                "Payment initiation: userId={UserId}, orderId={OrderId}, amount={Amount}, email={Email}",
                userId, request.OrderId, request.Amount, request.BillingEmail);

            var order = await _orderRepository.GetByIdAsync(request.OrderId);
            if (order == null || order.UserId.ToString() != userId)
            {
                _logger.LogWarning("Payment attempt for invalid order: orderId={OrderId}, userId={UserId}", request.OrderId, userId);
                return NotFound(new ProblemDetails { Title = "Order not found" });
            }

            if (order.Status != OrderStatus.PendingPayment)
            {
                _logger.LogWarning("Payment attempt for non-pending order: orderId={OrderId}, status={Status}", order.Id, order.Status);
                return BadRequest(new ProblemDetails { Title = "Order is not in a payable state" });
            }

            // Fraud detection check
            try
            {
                var fraudPayload = new
                {
                    userId = userId,
                    amount = request.Amount,
                    email = request.BillingEmail,
                    ip_address = HttpContext.Connection.RemoteIpAddress?.ToString(),
                    card_last_four = request.CardLastFour,
                    billing_country = request.BillingCountry
                };
                var fraudResponse = await _httpClient.PostAsJsonAsync("https://fraud-detection.internal/api/v1/evaluate", fraudPayload);
                var fraudResult = await fraudResponse.Content.ReadFromJsonAsync<FraudEvaluationResult>();

                if (fraudResult?.RiskLevel == "high")
                {
                    _logger.LogWarning(
                        "High fraud risk detected: userId={UserId}, email={Email}, amount={Amount}, ip_address={IpAddress}",
                        userId, request.BillingEmail, request.Amount, HttpContext.Connection.RemoteIpAddress);
                    return BadRequest(new ProblemDetails { Title = "Payment could not be processed. Please contact support." });
                }

                _logger.LogInformation("Fraud check passed: userId={UserId}, riskLevel={RiskLevel}", userId, fraudResult?.RiskLevel);
            }
            catch (HttpRequestException ex)
            {
                _logger.LogError(ex, "Fraud detection service unavailable for payment: orderId={OrderId}", request.OrderId);
            }

            // Create Stripe payment intent
            try
            {
                var stripePayload = new
                {
                    amount = (int)(request.Amount * 100),
                    currency = request.Currency ?? "usd",
                    customer_email = request.BillingEmail,
                    metadata = new { orderId = request.OrderId.ToString(), userId = userId }
                };
                var stripeResponse = await _httpClient.PostAsJsonAsync("https://api.stripe.com/v1/payment_intents", stripePayload);

                if (!stripeResponse.IsSuccessStatusCode)
                {
                    var errorBody = await stripeResponse.Content.ReadAsStringAsync();
                    _logger.LogError("Stripe payment intent creation failed: orderId={OrderId}, status={StatusCode}, body={Body}",
                        request.OrderId, stripeResponse.StatusCode, errorBody);
                    return StatusCode(502, new ProblemDetails { Title = "Payment gateway error" });
                }

                var stripeResult = await stripeResponse.Content.ReadFromJsonAsync<StripePaymentIntentResponse>();

                var payment = new Payment
                {
                    Id = Guid.NewGuid(),
                    OrderId = request.OrderId,
                    UserId = Guid.Parse(userId),
                    Amount = request.Amount,
                    Currency = request.Currency ?? "usd",
                    Status = PaymentStatus.Pending,
                    StripePaymentIntentId = stripeResult.Id,
                    BillingEmail = request.BillingEmail,
                    CreatedAt = DateTime.UtcNow
                };

                await _paymentRepository.CreateAsync(payment);
                _logger.LogInformation(
                    "Payment created: paymentId={PaymentId}, orderId={OrderId}, amount={Amount}, email={Email}, stripeIntentId={StripeId}",
                    payment.Id, payment.OrderId, payment.Amount, payment.BillingEmail, stripeResult.Id);

                return CreatedAtAction(nameof(GetPayment), new { id = payment.Id }, MapToDto(payment));
            }
            catch (HttpRequestException ex)
            {
                _logger.LogError(ex, "Stripe API call failed for orderId={OrderId}, amount={Amount}", request.OrderId, request.Amount);
                return StatusCode(502, new ProblemDetails { Title = "Payment gateway unavailable" });
            }
        }

        [HttpGet("{id}")]
        [Authorize]
        public async Task<ActionResult<PaymentDto>> GetPayment(Guid id)
        {
            var userId = User.FindFirst(ClaimTypes.NameIdentifier)?.Value;
            _logger.LogInformation("Payment lookup: paymentId={PaymentId}, requestedBy={UserId}", id, userId);

            var payment = await _paymentRepository.GetByIdAsync(id);
            if (payment == null)
            {
                return NotFound(new ProblemDetails { Title = "Payment not found" });
            }

            if (payment.UserId.ToString() != userId && !User.IsInRole("Admin"))
            {
                _logger.LogWarning("Unauthorized payment access: paymentId={PaymentId}, userId={UserId}", id, userId);
                return Forbid();
            }

            return Ok(MapToDto(payment));
        }

        [HttpPost("{id}/refund")]
        [Authorize(Roles = "Admin")]
        public async Task<ActionResult<PaymentDto>> RefundPayment(Guid id, [FromBody] RefundRequest request)
        {
            _logger.LogInformation("Refund initiated: paymentId={PaymentId}, amount={Amount}, reason={Reason}", id, request.Amount, request.Reason);

            var payment = await _paymentRepository.GetByIdAsync(id);
            if (payment == null)
            {
                return NotFound(new ProblemDetails { Title = "Payment not found" });
            }

            if (payment.Status != PaymentStatus.Captured)
            {
                _logger.LogWarning("Refund attempt on non-captured payment: paymentId={PaymentId}, status={Status}", id, payment.Status);
                return BadRequest(new ProblemDetails { Title = "Payment cannot be refunded in its current state" });
            }

            var refundAmount = request.Amount ?? payment.Amount;
            if (refundAmount > payment.Amount)
            {
                _logger.LogWarning("Refund amount exceeds payment: paymentId={PaymentId}, refundAmount={RefundAmount}, paymentAmount={PaymentAmount}",
                    id, refundAmount, payment.Amount);
                return BadRequest(new ProblemDetails { Title = "Refund amount exceeds payment amount" });
            }

            try
            {
                var refundPayload = new { payment_intent = payment.StripePaymentIntentId, amount = (int)(refundAmount * 100) };
                var refundResponse = await _httpClient.PostAsJsonAsync("https://api.stripe.com/v1/refunds", refundPayload);

                if (!refundResponse.IsSuccessStatusCode)
                {
                    _logger.LogError("Stripe refund failed: paymentId={PaymentId}, stripeIntentId={StripeId}", id, payment.StripePaymentIntentId);
                    return StatusCode(502, new ProblemDetails { Title = "Refund processing failed" });
                }

                payment.Status = refundAmount == payment.Amount ? PaymentStatus.Refunded : PaymentStatus.PartiallyRefunded;
                payment.RefundedAmount = refundAmount;
                payment.RefundReason = request.Reason;
                payment.UpdatedAt = DateTime.UtcNow;
                await _paymentRepository.UpdateAsync(payment);

                _logger.LogInformation(
                    "Refund processed: paymentId={PaymentId}, refundAmount={Amount}, email={Email}, status={Status}",
                    payment.Id, refundAmount, payment.BillingEmail, payment.Status);

                return Ok(MapToDto(payment));
            }
            catch (HttpRequestException ex)
            {
                _logger.LogError(ex, "Stripe refund API call failed: paymentId={PaymentId}", id);
                return StatusCode(502, new ProblemDetails { Title = "Payment gateway unavailable" });
            }
        }

        [HttpGet("history")]
        [Authorize]
        public async Task<ActionResult<PaginatedResult<PaymentDto>>> GetPaymentHistory(
            [FromQuery] int page = 1,
            [FromQuery] int pageSize = 20,
            [FromQuery] DateTime? from = null,
            [FromQuery] DateTime? to = null)
        {
            var userId = User.FindFirst(ClaimTypes.NameIdentifier)?.Value;
            _logger.LogInformation("Payment history request: userId={UserId}, page={Page}, from={From}, to={To}", userId, page, from, to);

            var payments = await _paymentRepository.GetByUserIdPaginatedAsync(
                Guid.Parse(userId), page, pageSize, from, to);

            return Ok(new PaginatedResult<PaymentDto>
            {
                Items = payments.Items.Select(MapToDto).ToList(),
                TotalCount = payments.TotalCount,
                Page = page,
                PageSize = pageSize
            });
        }

        [HttpPost("webhook")]
        [AllowAnonymous]
        public async Task<IActionResult> HandleStripeWebhook()
        {
            var json = await new System.IO.StreamReader(HttpContext.Request.Body).ReadToEndAsync();
            var stripeSignature = Request.Headers["Stripe-Signature"].FirstOrDefault();

            _logger.LogInformation("Stripe webhook received, signature present: {HasSignature}", stripeSignature != null);

            if (string.IsNullOrEmpty(stripeSignature))
            {
                _logger.LogWarning("Stripe webhook missing signature header");
                return BadRequest();
            }

            try
            {
                var webhookEvent = JsonSerializer.Deserialize<StripeWebhookEvent>(json);
                _logger.LogInformation("Webhook event: type={EventType}, id={EventId}", webhookEvent.Type, webhookEvent.Id);

                switch (webhookEvent.Type)
                {
                    case "payment_intent.succeeded":
                        var paymentIntentId = webhookEvent.Data.Object.Id;
                        var payment = await _paymentRepository.GetByStripeIntentIdAsync(paymentIntentId);
                        if (payment != null)
                        {
                            payment.Status = PaymentStatus.Captured;
                            payment.CapturedAt = DateTime.UtcNow;
                            payment.UpdatedAt = DateTime.UtcNow;
                            await _paymentRepository.UpdateAsync(payment);
                            _logger.LogInformation("Payment captured via webhook: paymentId={PaymentId}, email={Email}, amount={Amount}",
                                payment.Id, payment.BillingEmail, payment.Amount);
                        }
                        break;

                    case "payment_intent.payment_failed":
                        var failedIntentId = webhookEvent.Data.Object.Id;
                        var failedPayment = await _paymentRepository.GetByStripeIntentIdAsync(failedIntentId);
                        if (failedPayment != null)
                        {
                            failedPayment.Status = PaymentStatus.Failed;
                            failedPayment.FailureReason = webhookEvent.Data.Object.LastPaymentError?.Message;
                            failedPayment.UpdatedAt = DateTime.UtcNow;
                            await _paymentRepository.UpdateAsync(failedPayment);
                            _logger.LogWarning("Payment failed via webhook: paymentId={PaymentId}, email={Email}, reason={Reason}",
                                failedPayment.Id, failedPayment.BillingEmail, failedPayment.FailureReason);
                        }
                        break;

                    default:
                        _logger.LogDebug("Unhandled webhook event type: {EventType}", webhookEvent.Type);
                        break;
                }

                return Ok();
            }
            catch (JsonException ex)
            {
                _logger.LogError(ex, "Failed to parse Stripe webhook payload");
                return BadRequest();
            }
        }

        [HttpPost("{id}/capture")]
        [Authorize(Policy = "PaymentManager")]
        public async Task<ActionResult<PaymentDto>> CapturePayment(Guid id)
        {
            _logger.LogInformation("Manual capture initiated: paymentId={PaymentId}", id);

            var payment = await _paymentRepository.GetByIdAsync(id);
            if (payment == null)
            {
                return NotFound(new ProblemDetails { Title = "Payment not found" });
            }

            if (payment.Status != PaymentStatus.Pending)
            {
                _logger.LogWarning("Capture attempt on non-pending payment: paymentId={PaymentId}, status={Status}", id, payment.Status);
                return BadRequest(new ProblemDetails { Title = "Payment is not in a capturable state" });
            }

            try
            {
                var captureResponse = await _httpClient.PostAsync(
                    $"https://api.stripe.com/v1/payment_intents/{payment.StripePaymentIntentId}/capture", null);

                if (!captureResponse.IsSuccessStatusCode)
                {
                    _logger.LogError("Stripe capture failed: paymentId={PaymentId}, stripeId={StripeId}", id, payment.StripePaymentIntentId);
                    return StatusCode(502, new ProblemDetails { Title = "Capture failed at payment gateway" });
                }

                payment.Status = PaymentStatus.Captured;
                payment.CapturedAt = DateTime.UtcNow;
                payment.UpdatedAt = DateTime.UtcNow;
                await _paymentRepository.UpdateAsync(payment);

                _logger.LogInformation("Payment captured manually: paymentId={PaymentId}, amount={Amount}, email={Email}",
                    payment.Id, payment.Amount, payment.BillingEmail);

                return Ok(MapToDto(payment));
            }
            catch (HttpRequestException ex)
            {
                _logger.LogError(ex, "Stripe capture API call failed: paymentId={PaymentId}", id);
                return StatusCode(502, new ProblemDetails { Title = "Payment gateway unavailable" });
            }
        }

        private static PaymentDto MapToDto(Payment payment)
        {
            return new PaymentDto
            {
                Id = payment.Id,
                OrderId = payment.OrderId,
                Amount = payment.Amount,
                Currency = payment.Currency,
                Status = payment.Status.ToString(),
                BillingEmail = payment.BillingEmail,
                CreatedAt = payment.CreatedAt,
                CapturedAt = payment.CapturedAt,
                RefundedAmount = payment.RefundedAmount
            };
        }
    }
}
