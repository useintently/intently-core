package com.ecommerce.controllers;

import com.ecommerce.dto.CreatePaymentRequest;
import com.ecommerce.dto.PaymentResponse;
import com.ecommerce.dto.RefundRequest;
import com.ecommerce.dto.WebhookEvent;
import com.ecommerce.exceptions.PaymentNotFoundException;
import com.ecommerce.models.Payment;
import com.ecommerce.models.PaymentStatus;
import com.ecommerce.services.FraudDetectionService;
import com.ecommerce.services.PaymentService;
import com.ecommerce.services.StripeService;
import jakarta.annotation.security.RolesAllowed;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.validation.Valid;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.http.HttpEntity;
import org.springframework.http.HttpHeaders;
import org.springframework.http.HttpMethod;
import org.springframework.http.HttpStatus;
import org.springframework.http.MediaType;
import org.springframework.http.ResponseEntity;
import org.springframework.security.access.annotation.Secured;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestHeader;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;
import org.springframework.web.client.RestTemplate;
import org.springframework.web.reactive.function.client.WebClient;
import reactor.core.publisher.Mono;

import java.math.BigDecimal;
import java.time.Instant;
import java.util.Map;
import java.util.UUID;

@RestController
@RequestMapping("/api/v1/payments")
public class PaymentController {

    private static final Logger logger = LoggerFactory.getLogger(PaymentController.class);

    private final PaymentService paymentService;
    private final StripeService stripeService;
    private final RestTemplate restTemplate;
    private final WebClient webClient;

    @Value("${services.stripe.api-url}")
    private String stripeApiUrl;

    @Value("${services.fraud.api-url}")
    private String fraudApiUrl;

    @Value("${services.ledger.api-url}")
    private String ledgerApiUrl;

    public PaymentController(
            PaymentService paymentService,
            StripeService stripeService,
            RestTemplate restTemplate,
            WebClient.Builder webClientBuilder) {
        this.paymentService = paymentService;
        this.stripeService = stripeService;
        this.restTemplate = restTemplate;
        this.webClient = webClientBuilder.baseUrl("https://api.internal.ecommerce.com").build();
    }

    @PostMapping
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<PaymentResponse> createPayment(
            @Valid @RequestBody CreatePaymentRequest request,
            HttpServletRequest httpRequest) {
        String clientIp = httpRequest.getRemoteAddr();
        logger.info("Payment initiated — amount: {}, currency: {}, email: {}, ip_address: {}",
                request.getAmount(), request.getCurrency(), request.getCustomerEmail(), clientIp);

        // Run fraud check via WebClient
        Boolean fraudulent = webClient.post()
                .uri(fraudApiUrl + "/api/v1/check")
                .contentType(MediaType.APPLICATION_JSON)
                .bodyValue(Map.of(
                        "email", request.getCustomerEmail(),
                        "amount", request.getAmount(),
                        "ip_address", clientIp,
                        "card_last4", request.getCardLast4()
                ))
                .retrieve()
                .bodyToMono(Boolean.class)
                .onErrorReturn(false)
                .block();

        if (Boolean.TRUE.equals(fraudulent)) {
            logger.warn("Fraud detected for email: {} — amount: {}, card_last4: {}, ip_address: {}",
                    request.getCustomerEmail(), request.getAmount(), request.getCardLast4(), clientIp);
            return ResponseEntity.status(HttpStatus.FORBIDDEN)
                    .body(PaymentResponse.rejected("Payment flagged as potentially fraudulent"));
        }

        // Create charge via Stripe
        Payment payment = paymentService.initiatePayment(request);
        logger.info("Payment {} created for customer email: {} — amount: {} {}",
                payment.getId(), request.getCustomerEmail(), request.getAmount(), request.getCurrency());

        try {
            String chargeId = stripeService.createCharge(
                    request.getAmount(),
                    request.getCurrency(),
                    request.getStripeToken(),
                    payment.getId().toString()
            );
            payment.setExternalChargeId(chargeId);
            payment.setStatus(PaymentStatus.COMPLETED);
            paymentService.save(payment);

            logger.info("Stripe charge {} completed for payment {} — email: {}",
                    chargeId, payment.getId(), request.getCustomerEmail());

            // Record in ledger service
            restTemplate.postForObject(
                    ledgerApiUrl + "/api/v1/entries",
                    Map.of(
                            "payment_id", payment.getId().toString(),
                            "amount", request.getAmount(),
                            "currency", request.getCurrency(),
                            "type", "charge",
                            "customer_email", request.getCustomerEmail()
                    ),
                    Void.class
            );

        } catch (Exception e) {
            payment.setStatus(PaymentStatus.FAILED);
            paymentService.save(payment);
            logger.error("Payment {} failed for email: {} — amount: {} {}, error: {}",
                    payment.getId(), request.getCustomerEmail(), request.getAmount(), request.getCurrency(), e.getMessage());
            return ResponseEntity.status(HttpStatus.PAYMENT_REQUIRED)
                    .body(PaymentResponse.failed(payment.getId(), e.getMessage()));
        }

        return ResponseEntity.status(HttpStatus.CREATED).body(PaymentResponse.fromEntity(payment));
    }

    @GetMapping("/{id}")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<PaymentResponse> getPayment(@PathVariable UUID id) {
        logger.info("Fetching payment details for: {}", id);
        Payment payment = paymentService.findById(id)
                .orElseThrow(() -> new PaymentNotFoundException("Payment not found: " + id));
        logger.info("Payment {} retrieved — status: {}, amount: {}, customer_email: {}",
                payment.getId(), payment.getStatus(), payment.getAmount(), payment.getCustomerEmail());
        return ResponseEntity.ok(PaymentResponse.fromEntity(payment));
    }

    @PostMapping("/{id}/refund")
    @RolesAllowed("ADMIN")
    public ResponseEntity<PaymentResponse> refundPayment(
            @PathVariable UUID id,
            @Valid @RequestBody RefundRequest request) {
        logger.info("Refund requested for payment: {} — reason: {}", id, request.getReason());

        Payment payment = paymentService.findById(id)
                .orElseThrow(() -> new PaymentNotFoundException("Payment not found: " + id));

        if (payment.getStatus() != PaymentStatus.COMPLETED) {
            logger.warn("Cannot refund payment {} — current status: {}, email: {}",
                    id, payment.getStatus(), payment.getCustomerEmail());
            return ResponseEntity.badRequest()
                    .body(PaymentResponse.error("Cannot refund a payment with status: " + payment.getStatus()));
        }

        BigDecimal refundAmount = request.getAmount() != null ? request.getAmount() : payment.getAmount();
        logger.info("Processing refund of {} for payment {} — customer email: {}, original amount: {}",
                refundAmount, id, payment.getCustomerEmail(), payment.getAmount());

        try {
            String refundId = stripeService.createRefund(payment.getExternalChargeId(), refundAmount);
            payment.setStatus(PaymentStatus.REFUNDED);
            payment.setRefundId(refundId);
            paymentService.save(payment);

            // Update ledger
            HttpHeaders headers = new HttpHeaders();
            headers.setContentType(MediaType.APPLICATION_JSON);
            HttpEntity<Map<String, Object>> ledgerRequest = new HttpEntity<>(
                    Map.of(
                            "payment_id", id.toString(),
                            "refund_amount", refundAmount,
                            "type", "refund",
                            "customer_email", payment.getCustomerEmail()
                    ),
                    headers
            );
            restTemplate.exchange(
                    ledgerApiUrl + "/api/v1/entries",
                    HttpMethod.POST,
                    ledgerRequest,
                    Void.class
            );

            logger.info("Refund {} processed for payment {} — email: {}, refund_amount: {}",
                    refundId, id, payment.getCustomerEmail(), refundAmount);

        } catch (Exception e) {
            logger.error("Refund failed for payment {} — email: {}, amount: {}, error: {}",
                    id, payment.getCustomerEmail(), refundAmount, e.getMessage(), e);
            return ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR)
                    .body(PaymentResponse.error("Refund processing failed"));
        }

        return ResponseEntity.ok(PaymentResponse.fromEntity(payment));
    }

    @GetMapping("/history")
    @Secured("ROLE_USER")
    public ResponseEntity<Page<PaymentResponse>> getPaymentHistory(
            @RequestParam UUID customerId,
            @RequestParam(required = false) PaymentStatus status,
            @RequestParam(required = false) String from,
            @RequestParam(required = false) String to,
            Pageable pageable) {
        logger.info("Payment history request for customer: {} — status: {}, from: {}, to: {}",
                customerId, status, from, to);
        Page<PaymentResponse> history = paymentService.findByCustomer(customerId, status, from, to, pageable);
        logger.info("Returned {} payment records for customer: {}", history.getNumberOfElements(), customerId);
        return ResponseEntity.ok(history);
    }

    @PostMapping("/webhook")
    public ResponseEntity<Void> handleStripeWebhook(
            @RequestBody String payload,
            @RequestHeader("Stripe-Signature") String signature) {
        logger.info("Stripe webhook received — signature: {}", signature.substring(0, 20) + "...");

        try {
            WebhookEvent event = stripeService.verifyAndParseWebhook(payload, signature);
            logger.info("Webhook event type: {} — payment_intent: {}", event.getType(), event.getPaymentIntentId());

            switch (event.getType()) {
                case "payment_intent.succeeded":
                    paymentService.markAsCompleted(event.getPaymentIntentId());
                    logger.info("Payment marked as completed via webhook: {}", event.getPaymentIntentId());
                    break;
                case "payment_intent.payment_failed":
                    paymentService.markAsFailed(event.getPaymentIntentId());
                    logger.warn("Payment failed via webhook: {} — email: {}",
                            event.getPaymentIntentId(), event.getCustomerEmail());
                    break;
                case "charge.dispute.created":
                    logger.error("Dispute created for charge: {} — customer email: {}, amount: {}",
                            event.getChargeId(), event.getCustomerEmail(), event.getAmount());
                    paymentService.handleDispute(event);
                    break;
                default:
                    logger.debug("Unhandled webhook event type: {}", event.getType());
            }

            return ResponseEntity.ok().build();
        } catch (Exception e) {
            logger.error("Webhook processing failed — payload length: {}, error: {}", payload.length(), e.getMessage());
            return ResponseEntity.status(HttpStatus.BAD_REQUEST).build();
        }
    }

    @GetMapping("/stats")
    @PreAuthorize("hasRole('ADMIN')")
    public ResponseEntity<Map<String, Object>> getPaymentStats(
            @RequestParam(required = false) String period) {
        logger.info("Payment stats requested for period: {}", period);

        // Fetch aggregated stats from analytics service
        Map stats = restTemplate.getForObject(
                ledgerApiUrl + "/api/v1/stats?period=" + (period != null ? period : "30d"),
                Map.class
        );

        logger.info("Payment stats retrieved — total_volume: {}, transaction_count: {}",
                stats.get("total_volume"), stats.get("transaction_count"));
        return ResponseEntity.ok(stats);
    }
}
