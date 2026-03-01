package com.ecommerce.controllers

import com.ecommerce.dto.CreatePaymentRequest
import com.ecommerce.dto.PaymentResponse
import com.ecommerce.dto.RefundRequest
import com.ecommerce.exceptions.PaymentNotFoundException
import com.ecommerce.models.PaymentStatus
import com.ecommerce.services.PaymentService
import com.ecommerce.services.StripeService
import jakarta.annotation.security.RolesAllowed
import jakarta.servlet.http.HttpServletRequest
import jakarta.validation.Valid
import org.slf4j.LoggerFactory
import org.springframework.beans.factory.annotation.Value
import org.springframework.data.domain.Page
import org.springframework.data.domain.Pageable
import org.springframework.http.HttpEntity
import org.springframework.http.HttpHeaders
import org.springframework.http.HttpMethod
import org.springframework.http.HttpStatus
import org.springframework.http.MediaType
import org.springframework.http.ResponseEntity
import org.springframework.security.access.annotation.Secured
import org.springframework.security.access.prepost.PreAuthorize
import org.springframework.web.bind.annotation.GetMapping
import org.springframework.web.bind.annotation.PathVariable
import org.springframework.web.bind.annotation.PostMapping
import org.springframework.web.bind.annotation.RequestBody
import org.springframework.web.bind.annotation.RequestHeader
import org.springframework.web.bind.annotation.RequestMapping
import org.springframework.web.bind.annotation.RequestParam
import org.springframework.web.bind.annotation.RestController
import org.springframework.web.client.RestTemplate
import org.springframework.web.reactive.function.client.WebClient
import java.math.BigDecimal
import java.util.UUID

@RestController
@RequestMapping("/api/v1/payments")
class PaymentController(
    private val paymentService: PaymentService,
    private val stripeService: StripeService,
    private val restTemplate: RestTemplate,
    webClientBuilder: WebClient.Builder,
    @Value("\${services.fraud.api-url}") private val fraudApiUrl: String,
    @Value("\${services.ledger.api-url}") private val ledgerApiUrl: String
) {

    private val logger = LoggerFactory.getLogger(PaymentController::class.java)
    private val webClient: WebClient = webClientBuilder
        .baseUrl("https://api.internal.ecommerce.com")
        .build()

    @PostMapping
    @PreAuthorize("isAuthenticated()")
    fun createPayment(
        @Valid @RequestBody request: CreatePaymentRequest,
        httpRequest: HttpServletRequest
    ): ResponseEntity<PaymentResponse> {
        val clientIp = httpRequest.remoteAddr
        logger.info(
            "Payment initiated — amount: {}, currency: {}, email: {}, ip_address: {}",
            request.amount, request.currency, request.customerEmail, clientIp
        )

        // Fraud check via WebClient
        val fraudulent = webClient.post()
            .uri("$fraudApiUrl/api/v1/check")
            .contentType(MediaType.APPLICATION_JSON)
            .bodyValue(
                mapOf(
                    "email" to request.customerEmail,
                    "amount" to request.amount,
                    "ip_address" to clientIp,
                    "card_last4" to request.cardLast4
                )
            )
            .retrieve()
            .bodyToMono(Boolean::class.java)
            .onErrorReturn(false)
            .block() ?: false

        if (fraudulent) {
            logger.warn(
                "Fraud detected — email: {}, amount: {}, card_last4: {}, ip_address: {}",
                request.customerEmail, request.amount, request.cardLast4, clientIp
            )
            return ResponseEntity.status(HttpStatus.FORBIDDEN)
                .body(PaymentResponse.rejected("Payment flagged as fraudulent"))
        }

        val payment = paymentService.initiatePayment(request)
        logger.info("Payment {} created — email: {}, amount: {} {}",
            payment.id, request.customerEmail, request.amount, request.currency)

        return try {
            val chargeId = stripeService.createCharge(
                request.amount,
                request.currency,
                request.stripeToken,
                payment.id.toString()
            )
            payment.externalChargeId = chargeId
            payment.status = PaymentStatus.COMPLETED
            paymentService.save(payment)

            logger.info("Stripe charge {} completed — payment: {}, email: {}",
                chargeId, payment.id, request.customerEmail)

            // Record in ledger
            restTemplate.postForObject(
                "$ledgerApiUrl/api/v1/entries",
                mapOf(
                    "payment_id" to payment.id.toString(),
                    "amount" to request.amount,
                    "currency" to request.currency,
                    "type" to "charge",
                    "customer_email" to request.customerEmail
                ),
                Void::class.java
            )

            ResponseEntity.status(HttpStatus.CREATED).body(PaymentResponse.fromEntity(payment))
        } catch (e: Exception) {
            payment.status = PaymentStatus.FAILED
            paymentService.save(payment)
            logger.error("Payment {} failed — email: {}, amount: {} {}, error: {}",
                payment.id, request.customerEmail, request.amount, request.currency, e.message)
            ResponseEntity.status(HttpStatus.PAYMENT_REQUIRED)
                .body(PaymentResponse.failed(payment.id, e.message))
        }
    }

    @GetMapping("/{id}")
    @PreAuthorize("isAuthenticated()")
    fun getPayment(@PathVariable id: UUID): ResponseEntity<PaymentResponse> {
        logger.info("Fetching payment: {}", id)
        val payment = paymentService.findById(id)
            ?: throw PaymentNotFoundException("Payment not found: $id")
        logger.info("Payment {} — status: {}, amount: {}, email: {}",
            payment.id, payment.status, payment.amount, payment.customerEmail)
        return ResponseEntity.ok(PaymentResponse.fromEntity(payment))
    }

    @PostMapping("/{id}/refund")
    @PreAuthorize("hasRole('ADMIN')")
    fun refundPayment(
        @PathVariable id: UUID,
        @Valid @RequestBody request: RefundRequest
    ): ResponseEntity<PaymentResponse> {
        logger.info("Refund requested — payment: {}, reason: {}", id, request.reason)

        val payment = paymentService.findById(id)
            ?: throw PaymentNotFoundException("Payment not found: $id")

        if (payment.status != PaymentStatus.COMPLETED) {
            logger.warn("Cannot refund payment {} — status: {}, email: {}",
                id, payment.status, payment.customerEmail)
            return ResponseEntity.badRequest()
                .body(PaymentResponse.error("Cannot refund payment with status: ${payment.status}"))
        }

        val refundAmount = request.amount ?: payment.amount
        logger.info("Processing refund — payment: {}, amount: {}, email: {}", id, refundAmount, payment.customerEmail)

        return try {
            val refundId = stripeService.createRefund(payment.externalChargeId, refundAmount)
            payment.status = PaymentStatus.REFUNDED
            payment.refundId = refundId
            paymentService.save(payment)

            // Update ledger
            val headers = HttpHeaders().apply { contentType = MediaType.APPLICATION_JSON }
            val ledgerRequest = HttpEntity(
                mapOf(
                    "payment_id" to id.toString(),
                    "refund_amount" to refundAmount,
                    "type" to "refund",
                    "customer_email" to payment.customerEmail
                ),
                headers
            )
            restTemplate.exchange(
                "$ledgerApiUrl/api/v1/entries",
                HttpMethod.POST,
                ledgerRequest,
                Void::class.java
            )

            logger.info("Refund {} processed — payment: {}, email: {}, amount: {}",
                refundId, id, payment.customerEmail, refundAmount)
            ResponseEntity.ok(PaymentResponse.fromEntity(payment))
        } catch (e: Exception) {
            logger.error("Refund failed — payment: {}, email: {}, amount: {}, error: {}",
                id, payment.customerEmail, refundAmount, e.message, e)
            ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR)
                .body(PaymentResponse.error("Refund processing failed"))
        }
    }

    @GetMapping("/history")
    @Secured("ROLE_USER")
    fun getPaymentHistory(
        @RequestParam customerId: UUID,
        @RequestParam(required = false) status: PaymentStatus?,
        pageable: Pageable
    ): ResponseEntity<Page<PaymentResponse>> {
        logger.info("Payment history — customer: {}, status: {}", customerId, status)
        val history = paymentService.findByCustomer(customerId, status, pageable)
        logger.info("Returned {} records for customer: {}", history.numberOfElements, customerId)
        return ResponseEntity.ok(history)
    }

    @PostMapping("/webhook")
    fun handleStripeWebhook(
        @RequestBody payload: String,
        @RequestHeader("Stripe-Signature") signature: String
    ): ResponseEntity<Void> {
        logger.info("Stripe webhook — signature: {}...", signature.take(20))

        return try {
            val event = stripeService.verifyAndParseWebhook(payload, signature)
            logger.info("Webhook event: {} — payment_intent: {}", event.type, event.paymentIntentId)

            when (event.type) {
                "payment_intent.succeeded" -> {
                    paymentService.markAsCompleted(event.paymentIntentId)
                    logger.info("Payment completed via webhook: {}", event.paymentIntentId)
                }
                "payment_intent.payment_failed" -> {
                    paymentService.markAsFailed(event.paymentIntentId)
                    logger.warn("Payment failed via webhook: {} — email: {}", event.paymentIntentId, event.customerEmail)
                }
                "charge.dispute.created" -> {
                    logger.error("Dispute — charge: {}, email: {}, amount: {}",
                        event.chargeId, event.customerEmail, event.amount)
                    paymentService.handleDispute(event)
                }
                else -> logger.debug("Unhandled webhook event: {}", event.type)
            }

            ResponseEntity.ok().build()
        } catch (e: Exception) {
            logger.error("Webhook processing failed: {}", e.message)
            ResponseEntity.status(HttpStatus.BAD_REQUEST).build()
        }
    }

    @GetMapping("/stats")
    @RolesAllowed("ADMIN")
    fun getPaymentStats(@RequestParam(required = false) period: String?): ResponseEntity<Map<String, Any>> {
        val queryPeriod = period ?: "30d"
        logger.info("Payment stats — period: {}", queryPeriod)

        val stats = restTemplate.getForObject(
            "$ledgerApiUrl/api/v1/stats?period=$queryPeriod",
            Map::class.java
        ) as Map<String, Any>

        logger.info("Stats retrieved — volume: {}, count: {}", stats["total_volume"], stats["transaction_count"])
        return ResponseEntity.ok(stats)
    }
}
