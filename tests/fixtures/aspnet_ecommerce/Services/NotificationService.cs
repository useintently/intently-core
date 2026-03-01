using System;
using System.Collections.Generic;
using System.Net.Http;
using System.Net.Http.Json;
using System.Threading.Tasks;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;
using EcommerceApi.Configuration;
using EcommerceApi.Models;

namespace EcommerceApi.Services
{
    public class NotificationService : INotificationService
    {
        private readonly HttpClient _httpClient;
        private readonly ILogger<NotificationService> _logger;
        private readonly NotificationSettings _settings;
        private readonly ITemplateRenderer _templateRenderer;

        public NotificationService(
            HttpClient httpClient,
            ILogger<NotificationService> logger,
            IOptions<NotificationSettings> settings,
            ITemplateRenderer templateRenderer)
        {
            _httpClient = httpClient;
            _logger = logger;
            _settings = settings.Value;
            _templateRenderer = templateRenderer;
        }

        public async Task<bool> SendEmailAsync(
            string recipientEmail,
            string recipientName,
            string subject,
            string templateId,
            Dictionary<string, string>? templateData = null)
        {
            _logger.LogInformation(
                "Sending email: to={Email}, name={Name}, subject={Subject}, template={TemplateId}",
                recipientEmail, recipientName, subject, templateId);

            var htmlBody = await _templateRenderer.RenderAsync(templateId, templateData);

            var sendGridPayload = new
            {
                personalizations = new[]
                {
                    new
                    {
                        to = new[] { new { email = recipientEmail, name = recipientName } },
                        subject = subject,
                        dynamic_template_data = templateData
                    }
                },
                from = new { email = _settings.FromEmail, name = _settings.FromName },
                content = new[]
                {
                    new { type = "text/html", value = htmlBody }
                }
            };

            try
            {
                _httpClient.DefaultRequestHeaders.Clear();
                _httpClient.DefaultRequestHeaders.Add("Authorization", $"Bearer {_settings.SendGridApiKey}");

                var response = await _httpClient.PostAsJsonAsync("https://api.sendgrid.com/v3/mail/send", sendGridPayload);

                if (!response.IsSuccessStatusCode)
                {
                    var errorBody = await response.Content.ReadAsStringAsync();
                    _logger.LogError(
                        "SendGrid email failed: to={Email}, name={Name}, status={StatusCode}, error={Error}",
                        recipientEmail, recipientName, response.StatusCode, errorBody);
                    return false;
                }

                _logger.LogInformation("Email sent successfully: to={Email}, name={Name}, subject={Subject}",
                    recipientEmail, recipientName, subject);
                return true;
            }
            catch (HttpRequestException ex)
            {
                _logger.LogError(ex,
                    "SendGrid API connection failed: to={Email}, name={Name}",
                    recipientEmail, recipientName);
                return false;
            }
        }

        public async Task<bool> SendSmsAsync(string phoneNumber, string message)
        {
            _logger.LogInformation("Sending SMS: phone={PhoneNumber}, messageLength={Length}", phoneNumber, message.Length);

            var twilioPayload = new Dictionary<string, string>
            {
                ["To"] = phoneNumber,
                ["From"] = _settings.TwilioFromNumber,
                ["Body"] = message
            };

            try
            {
                var request = new HttpRequestMessage(HttpMethod.Post,
                    $"https://api.twilio.com/2010-04-01/Accounts/{_settings.TwilioAccountSid}/Messages.json")
                {
                    Content = new FormUrlEncodedContent(twilioPayload)
                };

                var authBytes = System.Text.Encoding.ASCII.GetBytes(
                    $"{_settings.TwilioAccountSid}:{_settings.TwilioAuthToken}");
                request.Headers.Authorization =
                    new System.Net.Http.Headers.AuthenticationHeaderValue("Basic", Convert.ToBase64String(authBytes));

                var response = await _httpClient.SendAsync(request);

                if (!response.IsSuccessStatusCode)
                {
                    var errorBody = await response.Content.ReadAsStringAsync();
                    _logger.LogError("Twilio SMS failed: phone={PhoneNumber}, status={StatusCode}, error={Error}",
                        phoneNumber, response.StatusCode, errorBody);
                    return false;
                }

                _logger.LogInformation("SMS sent successfully: phone={PhoneNumber}", phoneNumber);
                return true;
            }
            catch (HttpRequestException ex)
            {
                _logger.LogError(ex, "Twilio API connection failed: phone={PhoneNumber}", phoneNumber);
                return false;
            }
        }

        public async Task<bool> SendOrderConfirmationAsync(Order order, string customerEmail, string customerName)
        {
            _logger.LogInformation(
                "Sending order confirmation: orderId={OrderId}, email={Email}, name={Name}, total={Total}",
                order.Id, customerEmail, customerName, order.TotalAmount);

            var templateData = new Dictionary<string, string>
            {
                ["order_id"] = order.Id.ToString(),
                ["customer_name"] = customerName,
                ["total_amount"] = order.TotalAmount.ToString("C"),
                ["order_date"] = order.CreatedAt.ToString("yyyy-MM-dd"),
                ["item_count"] = order.Items.Count.ToString()
            };

            var emailSent = await SendEmailAsync(
                customerEmail,
                customerName,
                $"Order Confirmation - #{order.Id.ToString()[..8]}",
                "order-confirmation",
                templateData);

            if (!emailSent)
            {
                _logger.LogWarning("Order confirmation email failed: orderId={OrderId}, email={Email}", order.Id, customerEmail);
            }

            return emailSent;
        }

        public async Task<bool> SendPasswordResetAsync(string email, string name, string resetToken, string resetUrl)
        {
            _logger.LogInformation("Sending password reset: email={Email}, name={Name}", email, name);

            var templateData = new Dictionary<string, string>
            {
                ["user_name"] = name,
                ["reset_url"] = $"{resetUrl}?token={resetToken}",
                ["expiry_hours"] = "1"
            };

            return await SendEmailAsync(email, name, "Password Reset Request", "password-reset", templateData);
        }

        public async Task<bool> SendShippingNotificationAsync(
            string email,
            string name,
            string phone,
            string orderId,
            string trackingNumber,
            string carrier)
        {
            _logger.LogInformation(
                "Sending shipping notification: email={Email}, name={Name}, phone={Phone}, orderId={OrderId}, tracking={TrackingNumber}",
                email, name, phone, orderId, trackingNumber);

            var templateData = new Dictionary<string, string>
            {
                ["customer_name"] = name,
                ["order_id"] = orderId,
                ["tracking_number"] = trackingNumber,
                ["carrier"] = carrier,
                ["tracking_url"] = $"https://track.example.com/{carrier}/{trackingNumber}"
            };

            var emailTask = SendEmailAsync(
                email,
                name,
                $"Your Order Has Shipped - #{orderId[..8]}",
                "shipping-notification",
                templateData);

            var smsMessage = $"Your order #{orderId[..8]} has shipped! Track: https://track.example.com/{carrier}/{trackingNumber}";
            var smsTask = SendSmsAsync(phone, smsMessage);

            var results = await Task.WhenAll(emailTask, smsTask);

            if (!results[0])
            {
                _logger.LogWarning("Shipping notification email failed: email={Email}, orderId={OrderId}", email, orderId);
            }

            if (!results[1])
            {
                _logger.LogWarning("Shipping notification SMS failed: phone={Phone}, orderId={OrderId}", phone, orderId);
            }

            return results[0] || results[1];
        }

        public async Task<bool> SendSecurityAlertAsync(string email, string name, string alertType, string ipAddress)
        {
            _logger.LogWarning(
                "Sending security alert: email={Email}, name={Name}, alertType={AlertType}, ip_address={IpAddress}",
                email, name, alertType, ipAddress);

            var templateData = new Dictionary<string, string>
            {
                ["user_name"] = name,
                ["alert_type"] = alertType,
                ["ip_address"] = ipAddress,
                ["timestamp"] = DateTime.UtcNow.ToString("yyyy-MM-dd HH:mm:ss UTC")
            };

            return await SendEmailAsync(
                email,
                name,
                $"Security Alert: {alertType}",
                "security-alert",
                templateData);
        }
    }
}
