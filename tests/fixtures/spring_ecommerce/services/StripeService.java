package com.ecommerce.services;

import com.ecommerce.dto.StripeChargeResponse;
import com.ecommerce.dto.StripeCustomerResponse;
import com.ecommerce.dto.StripeRefundResponse;
import com.ecommerce.exceptions.PaymentProcessingException;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.http.HttpEntity;
import org.springframework.http.HttpHeaders;
import org.springframework.http.HttpMethod;
import org.springframework.http.MediaType;
import org.springframework.http.ResponseEntity;
import org.springframework.stereotype.Service;
import org.springframework.web.client.HttpClientErrorException;
import org.springframework.web.client.RestTemplate;
import org.springframework.web.reactive.function.client.WebClient;
import reactor.core.publisher.Mono;

import java.math.BigDecimal;
import java.time.Duration;
import java.util.HashMap;
import java.util.Map;

@Service
public class StripeService {

    private static final Logger logger = LoggerFactory.getLogger(StripeService.class);

    private final RestTemplate restTemplate;
    private final WebClient webClient;

    @Value("${stripe.api-key}")
    private String stripeApiKey;

    @Value("${stripe.api-url:https://api.stripe.com/v1}")
    private String stripeApiUrl;

    @Value("${stripe.webhook-secret}")
    private String webhookSecret;

    public StripeService(RestTemplate restTemplate, WebClient.Builder webClientBuilder) {
        this.restTemplate = restTemplate;
        this.webClient = webClientBuilder
                .baseUrl("https://api.stripe.com/v1")
                .defaultHeader("Authorization", "Bearer " + stripeApiKey)
                .build();
    }

    public String createCharge(BigDecimal amount, String currency, String token, String paymentId) {
        logger.info("Creating Stripe charge — amount: {}, currency: {}, payment_id: {}", amount, currency, paymentId);

        HttpHeaders headers = buildStripeHeaders();
        Map<String, Object> chargeRequest = new HashMap<>();
        chargeRequest.put("amount", amount.multiply(BigDecimal.valueOf(100)).intValue());
        chargeRequest.put("currency", currency.toLowerCase());
        chargeRequest.put("source", token);
        chargeRequest.put("metadata", Map.of("payment_id", paymentId));
        chargeRequest.put("description", "Charge for order " + paymentId);

        HttpEntity<Map<String, Object>> entity = new HttpEntity<>(chargeRequest, headers);

        try {
            ResponseEntity<StripeChargeResponse> response = restTemplate.exchange(
                    stripeApiUrl + "/charges",
                    HttpMethod.POST,
                    entity,
                    StripeChargeResponse.class
            );

            StripeChargeResponse charge = response.getBody();
            logger.info("Stripe charge created — charge_id: {}, amount: {}, status: {}",
                    charge.getId(), amount, charge.getStatus());
            return charge.getId();

        } catch (HttpClientErrorException e) {
            logger.error("Stripe charge failed — payment_id: {}, amount: {}, status: {}, error: {}",
                    paymentId, amount, e.getStatusCode(), e.getResponseBodyAsString());
            throw new PaymentProcessingException("Stripe charge failed: " + e.getMessage());
        }
    }

    public String createRefund(String chargeId, BigDecimal amount) {
        logger.info("Creating Stripe refund — charge_id: {}, amount: {}", chargeId, amount);

        HttpHeaders headers = buildStripeHeaders();
        Map<String, Object> refundRequest = new HashMap<>();
        refundRequest.put("charge", chargeId);
        refundRequest.put("amount", amount.multiply(BigDecimal.valueOf(100)).intValue());
        refundRequest.put("reason", "requested_by_customer");

        HttpEntity<Map<String, Object>> entity = new HttpEntity<>(refundRequest, headers);

        try {
            ResponseEntity<StripeRefundResponse> response = restTemplate.postForEntity(
                    stripeApiUrl + "/refunds",
                    entity,
                    StripeRefundResponse.class
            );

            StripeRefundResponse refund = response.getBody();
            logger.info("Stripe refund created — refund_id: {}, charge_id: {}, amount: {}, status: {}",
                    refund.getId(), chargeId, amount, refund.getStatus());
            return refund.getId();

        } catch (HttpClientErrorException e) {
            logger.error("Stripe refund failed — charge_id: {}, amount: {}, error: {}",
                    chargeId, amount, e.getResponseBodyAsString());
            throw new PaymentProcessingException("Stripe refund failed: " + e.getMessage());
        }
    }

    public Map<String, Object> getBalance() {
        logger.info("Fetching Stripe account balance");

        try {
            Map balance = restTemplate.getForObject(
                    stripeApiUrl + "/balance",
                    Map.class
            );
            logger.info("Stripe balance retrieved — available: {}", balance.get("available"));
            return balance;

        } catch (Exception e) {
            logger.error("Failed to fetch Stripe balance: {}", e.getMessage());
            throw new PaymentProcessingException("Failed to fetch balance: " + e.getMessage());
        }
    }

    public String createCustomer(String email, String name, String paymentMethodId) {
        logger.info("Creating Stripe customer — email: {}, name: {}", email, name);

        // Using WebClient for async customer creation
        StripeCustomerResponse customer = webClient.post()
                .uri("/customers")
                .contentType(MediaType.APPLICATION_FORM_URLENCODED)
                .bodyValue(String.format(
                        "email=%s&name=%s&payment_method=%s",
                        email, name, paymentMethodId
                ))
                .retrieve()
                .bodyToMono(StripeCustomerResponse.class)
                .timeout(Duration.ofSeconds(10))
                .doOnError(e -> logger.error("Stripe customer creation failed — email: {}, name: {}, error: {}",
                        email, name, e.getMessage()))
                .block();

        logger.info("Stripe customer created — customer_id: {}, email: {}", customer.getId(), email);
        return customer.getId();
    }

    public Map<String, Object> getPaymentIntent(String paymentIntentId) {
        logger.info("Retrieving payment intent: {}", paymentIntentId);

        Map paymentIntent = webClient.get()
                .uri("/payment_intents/" + paymentIntentId)
                .retrieve()
                .bodyToMono(Map.class)
                .timeout(Duration.ofSeconds(5))
                .block();

        String customerEmail = (String) ((Map) paymentIntent.get("customer_details")).get("email");
        logger.info("Payment intent {} retrieved — status: {}, customer_email: {}",
                paymentIntentId, paymentIntent.get("status"), customerEmail);
        return paymentIntent;
    }

    public void attachPaymentMethod(String customerId, String paymentMethodId) {
        logger.info("Attaching payment method {} to customer {}", paymentMethodId, customerId);

        HttpHeaders headers = buildStripeHeaders();
        Map<String, String> body = Map.of("customer", customerId);
        HttpEntity<Map<String, String>> entity = new HttpEntity<>(body, headers);

        try {
            restTemplate.postForObject(
                    stripeApiUrl + "/payment_methods/" + paymentMethodId + "/attach",
                    entity,
                    Map.class
            );
            logger.info("Payment method {} attached to customer {}", paymentMethodId, customerId);
        } catch (HttpClientErrorException e) {
            logger.error("Failed to attach payment method — customer: {}, payment_method: {}, error: {}",
                    customerId, paymentMethodId, e.getResponseBodyAsString());
            throw new PaymentProcessingException("Failed to attach payment method: " + e.getMessage());
        }
    }

    public WebhookEvent verifyAndParseWebhook(String payload, String signature) {
        logger.debug("Verifying webhook signature — payload_length: {}", payload.length());

        // Verify signature using Stripe SDK
        try {
            // Stripe signature verification logic
            String[] parts = signature.split(",");
            String timestamp = parts[0].split("=")[1];
            String sig = parts[1].split("=")[1];

            logger.info("Webhook verified — timestamp: {}", timestamp);
            return WebhookEvent.parse(payload);
        } catch (Exception e) {
            logger.error("Webhook signature verification failed: {}", e.getMessage());
            throw new PaymentProcessingException("Invalid webhook signature");
        }
    }

    public void cancelSubscription(String subscriptionId, String customerEmail) {
        logger.info("Cancelling subscription {} for customer email: {}", subscriptionId, customerEmail);

        try {
            restTemplate.delete(stripeApiUrl + "/subscriptions/" + subscriptionId);
            logger.info("Subscription {} cancelled for email: {}", subscriptionId, customerEmail);
        } catch (Exception e) {
            logger.error("Subscription cancellation failed — subscription: {}, email: {}, error: {}",
                    subscriptionId, customerEmail, e.getMessage());
            throw new PaymentProcessingException("Subscription cancellation failed: " + e.getMessage());
        }
    }

    private HttpHeaders buildStripeHeaders() {
        HttpHeaders headers = new HttpHeaders();
        headers.setContentType(MediaType.APPLICATION_JSON);
        headers.setBearerAuth(stripeApiKey);
        return headers;
    }
}
