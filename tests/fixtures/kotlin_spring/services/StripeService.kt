package com.ecommerce.services

import com.ecommerce.dto.StripeChargeResponse
import com.ecommerce.dto.StripeCustomerResponse
import com.ecommerce.dto.StripeRefundResponse
import com.ecommerce.dto.WebhookEvent
import com.ecommerce.exceptions.PaymentProcessingException
import org.slf4j.LoggerFactory
import org.springframework.beans.factory.annotation.Value
import org.springframework.http.HttpEntity
import org.springframework.http.HttpHeaders
import org.springframework.http.HttpMethod
import org.springframework.http.MediaType
import org.springframework.stereotype.Service
import org.springframework.web.client.HttpClientErrorException
import org.springframework.web.client.RestTemplate
import org.springframework.web.reactive.function.client.WebClient
import java.math.BigDecimal
import java.time.Duration

@Service
class StripeService(
    private val restTemplate: RestTemplate,
    webClientBuilder: WebClient.Builder,
    @Value("\${stripe.api-key}") private val stripeApiKey: String,
    @Value("\${stripe.api-url:https://api.stripe.com/v1}") private val stripeApiUrl: String,
    @Value("\${stripe.webhook-secret}") private val webhookSecret: String
) {

    private val logger = LoggerFactory.getLogger(StripeService::class.java)
    private val webClient: WebClient = webClientBuilder
        .baseUrl("https://api.stripe.com/v1")
        .defaultHeader("Authorization", "Bearer $stripeApiKey")
        .build()

    fun createCharge(amount: BigDecimal, currency: String, token: String, paymentId: String): String {
        logger.info("Creating Stripe charge — amount: {}, currency: {}, payment_id: {}", amount, currency, paymentId)

        val headers = buildStripeHeaders()
        val chargeRequest = mapOf(
            "amount" to amount.multiply(BigDecimal.valueOf(100)).toInt(),
            "currency" to currency.lowercase(),
            "source" to token,
            "metadata" to mapOf("payment_id" to paymentId),
            "description" to "Charge for order $paymentId"
        )

        val entity = HttpEntity(chargeRequest, headers)

        return try {
            val response = restTemplate.exchange(
                "$stripeApiUrl/charges",
                HttpMethod.POST,
                entity,
                StripeChargeResponse::class.java
            )

            val charge = response.body!!
            logger.info("Stripe charge created — charge_id: {}, amount: {}, status: {}",
                charge.id, amount, charge.status)
            charge.id
        } catch (e: HttpClientErrorException) {
            logger.error("Stripe charge failed — payment_id: {}, amount: {}, status: {}, error: {}",
                paymentId, amount, e.statusCode, e.responseBodyAsString)
            throw PaymentProcessingException("Stripe charge failed: ${e.message}")
        }
    }

    fun createRefund(chargeId: String, amount: BigDecimal): String {
        logger.info("Creating Stripe refund — charge_id: {}, amount: {}", chargeId, amount)

        val headers = buildStripeHeaders()
        val refundRequest = mapOf(
            "charge" to chargeId,
            "amount" to amount.multiply(BigDecimal.valueOf(100)).toInt(),
            "reason" to "requested_by_customer"
        )

        val entity = HttpEntity(refundRequest, headers)

        return try {
            val response = restTemplate.postForEntity(
                "$stripeApiUrl/refunds",
                entity,
                StripeRefundResponse::class.java
            )

            val refund = response.body!!
            logger.info("Stripe refund created — refund_id: {}, charge_id: {}, amount: {}, status: {}",
                refund.id, chargeId, amount, refund.status)
            refund.id
        } catch (e: HttpClientErrorException) {
            logger.error("Stripe refund failed — charge_id: {}, amount: {}, error: {}",
                chargeId, amount, e.responseBodyAsString)
            throw PaymentProcessingException("Stripe refund failed: ${e.message}")
        }
    }

    fun getBalance(): Map<String, Any> {
        logger.info("Fetching Stripe account balance")

        return try {
            @Suppress("UNCHECKED_CAST")
            val balance = restTemplate.getForObject(
                "$stripeApiUrl/balance",
                Map::class.java
            ) as Map<String, Any>
            logger.info("Stripe balance retrieved — available: {}", balance["available"])
            balance
        } catch (e: Exception) {
            logger.error("Failed to fetch Stripe balance: {}", e.message)
            throw PaymentProcessingException("Failed to fetch balance: ${e.message}")
        }
    }

    fun createCustomer(email: String, name: String, paymentMethodId: String): String {
        logger.info("Creating Stripe customer — email: {}, name: {}", email, name)

        val customer = webClient.post()
            .uri("/customers")
            .contentType(MediaType.APPLICATION_FORM_URLENCODED)
            .bodyValue("email=$email&name=$name&payment_method=$paymentMethodId")
            .retrieve()
            .bodyToMono(StripeCustomerResponse::class.java)
            .timeout(Duration.ofSeconds(10))
            .doOnError { e ->
                logger.error("Stripe customer creation failed — email: {}, name: {}, error: {}",
                    email, name, e.message)
            }
            .block()!!

        logger.info("Stripe customer created — customer_id: {}, email: {}", customer.id, email)
        return customer.id
    }

    fun getPaymentIntent(paymentIntentId: String): Map<String, Any> {
        logger.info("Retrieving payment intent: {}", paymentIntentId)

        @Suppress("UNCHECKED_CAST")
        val paymentIntent = webClient.get()
            .uri("/payment_intents/$paymentIntentId")
            .retrieve()
            .bodyToMono(Map::class.java)
            .timeout(Duration.ofSeconds(5))
            .block() as Map<String, Any>

        @Suppress("UNCHECKED_CAST")
        val customerDetails = paymentIntent["customer_details"] as Map<String, Any>
        val customerEmail = customerDetails["email"] as String
        logger.info("Payment intent {} — status: {}, customer_email: {}",
            paymentIntentId, paymentIntent["status"], customerEmail)
        return paymentIntent
    }

    fun attachPaymentMethod(customerId: String, paymentMethodId: String) {
        logger.info("Attaching payment method {} to customer {}", paymentMethodId, customerId)

        val headers = buildStripeHeaders()
        val body = mapOf("customer" to customerId)
        val entity = HttpEntity(body, headers)

        try {
            restTemplate.postForObject(
                "$stripeApiUrl/payment_methods/$paymentMethodId/attach",
                entity,
                Map::class.java
            )
            logger.info("Payment method {} attached to customer {}", paymentMethodId, customerId)
        } catch (e: HttpClientErrorException) {
            logger.error("Failed to attach payment method — customer: {}, payment_method: {}, error: {}",
                customerId, paymentMethodId, e.responseBodyAsString)
            throw PaymentProcessingException("Failed to attach payment method: ${e.message}")
        }
    }

    fun verifyAndParseWebhook(payload: String, signature: String): WebhookEvent {
        logger.debug("Verifying webhook signature — payload_length: {}", payload.length)

        return try {
            val parts = signature.split(",")
            val timestamp = parts[0].split("=")[1]
            logger.info("Webhook verified — timestamp: {}", timestamp)
            WebhookEvent.parse(payload)
        } catch (e: Exception) {
            logger.error("Webhook signature verification failed: {}", e.message)
            throw PaymentProcessingException("Invalid webhook signature")
        }
    }

    fun cancelSubscription(subscriptionId: String, customerEmail: String) {
        logger.info("Cancelling subscription {} for email: {}", subscriptionId, customerEmail)

        try {
            restTemplate.delete("$stripeApiUrl/subscriptions/$subscriptionId")
            logger.info("Subscription {} cancelled for email: {}", subscriptionId, customerEmail)
        } catch (e: Exception) {
            logger.error("Subscription cancellation failed — subscription: {}, email: {}, error: {}",
                subscriptionId, customerEmail, e.message)
            throw PaymentProcessingException("Subscription cancellation failed: ${e.message}")
        }
    }

    private fun buildStripeHeaders(): HttpHeaders {
        return HttpHeaders().apply {
            contentType = MediaType.APPLICATION_JSON
            setBearerAuth(stripeApiKey)
        }
    }
}
