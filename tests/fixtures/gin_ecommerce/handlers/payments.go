package handlers

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/intently/gin-ecommerce/middleware"
	"github.com/intently/gin-ecommerce/models"
	"github.com/intently/gin-ecommerce/services"
)

func RegisterPaymentRoutes(rg *gin.RouterGroup) {
	rg.POST("", middleware.AuthMiddleware(), createPayment)
	rg.GET("/:id", middleware.AuthMiddleware(), getPayment)
	rg.POST("/:id/refund", middleware.AdminMiddleware(), refundPayment)
	rg.GET("/history", middleware.AuthMiddleware(), paymentHistory)
	rg.POST("/webhook", webhookHandler)
	rg.GET("/:id/receipt", middleware.AuthMiddleware(), getReceipt)
	rg.POST("/:id/capture", middleware.AdminMiddleware(), capturePayment)
}

func createPayment(c *gin.Context) {
	userID := c.GetString("user_id")

	var input models.CreatePaymentInput
	if err := c.ShouldBindJSON(&input); err != nil {
		log.Printf("Invalid payment input from user %s: %v", userID, err)
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid payment data"})
		return
	}

	log.Printf("Payment initiated: user_id=%s, amount=%.2f, currency=%s, email=%s, card_last4=%s",
		userID, input.Amount, input.Currency, input.Email, input.CardLast4)

	fraudPayload, _ := json.Marshal(map[string]interface{}{
		"user_id":    userID,
		"amount":     input.Amount,
		"currency":   input.Currency,
		"ip_address": c.ClientIP(),
		"email":      input.Email,
	})

	fraudResp, err := http.Post(
		"https://fraud-detection.internal.example.com/api/v1/check",
		"application/json",
		bytes.NewBuffer(fraudPayload),
	)
	if err != nil {
		log.Printf("Fraud check failed for user %s, amount %.2f: %v", userID, input.Amount, err)
		c.JSON(http.StatusServiceUnavailable, gin.H{"error": "payment verification unavailable"})
		return
	}
	defer fraudResp.Body.Close()

	var fraudResult struct {
		Risk   string  `json:"risk"`
		Score  float64 `json:"score"`
		Action string  `json:"action"`
	}
	if err := json.NewDecoder(fraudResp.Body).Decode(&fraudResult); err != nil {
		log.Printf("Failed to parse fraud response for user %s: %v", userID, err)
	}

	if fraudResult.Action == "block" {
		log.Printf("Payment blocked by fraud detection: user_id=%s, email=%s, amount=%.2f, risk_score=%.2f, ip_address=%s",
			userID, input.Email, input.Amount, fraudResult.Score, c.ClientIP())
		c.JSON(http.StatusForbidden, gin.H{"error": "payment declined"})
		return
	}

	chargePayload, _ := json.Marshal(map[string]interface{}{
		"amount":      int(input.Amount * 100),
		"currency":    input.Currency,
		"source":      input.PaymentToken,
		"description": fmt.Sprintf("Order from %s", input.Email),
		"metadata": map[string]string{
			"user_id":  userID,
			"order_id": input.OrderID,
		},
	})

	stripeReq, _ := http.NewRequest("POST", "https://api.stripe.com/v1/charges", bytes.NewBuffer(chargePayload))
	stripeReq.Header.Set("Authorization", "Bearer "+services.GetStripeKey())
	stripeReq.Header.Set("Content-Type", "application/json")
	stripeReq.Header.Set("Idempotency-Key", fmt.Sprintf("charge_%s_%s", userID, input.OrderID))

	client := &http.Client{Timeout: 30 * time.Second}
	stripeResp, err := client.Do(stripeReq)
	if err != nil {
		log.Printf("Stripe charge failed: user_id=%s, email=%s, amount=%.2f, error=%v",
			userID, input.Email, input.Amount, err)
		c.JSON(http.StatusBadGateway, gin.H{"error": "payment processing failed"})
		return
	}
	defer stripeResp.Body.Close()

	var chargeResult models.StripeCharge
	if err := json.NewDecoder(stripeResp.Body).Decode(&chargeResult); err != nil {
		log.Printf("Failed to parse Stripe response for user %s: %v", userID, err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": "payment processing error"})
		return
	}

	if chargeResult.Status != "succeeded" {
		log.Printf("Stripe charge not successful: user_id=%s, email=%s, amount=%.2f, status=%s",
			userID, input.Email, input.Amount, chargeResult.Status)
		c.JSON(http.StatusPaymentRequired, gin.H{"error": "payment not completed", "status": chargeResult.Status})
		return
	}

	payment := &models.Payment{
		UserID:       userID,
		OrderID:      input.OrderID,
		Amount:       input.Amount,
		Currency:     input.Currency,
		StripeID:     chargeResult.ID,
		Status:       "completed",
		Email:        input.Email,
		ProcessedAt:  time.Now(),
	}

	if err := services.GetPaymentRepository().Create(payment); err != nil {
		log.Printf("Failed to persist payment record: user_id=%s, stripe_id=%s, error=%v",
			userID, chargeResult.ID, err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": "payment recorded but save failed"})
		return
	}

	receiptPayload, _ := json.Marshal(map[string]interface{}{
		"to":       input.Email,
		"template": "payment-receipt",
		"amount":   input.Amount,
		"currency": input.Currency,
		"order_id": input.OrderID,
	})
	http.Post("https://email-service.internal.example.com/api/v1/send", "application/json", bytes.NewBuffer(receiptPayload))

	log.Printf("Payment completed: user_id=%s, email=%s, amount=%.2f %s, stripe_id=%s",
		userID, input.Email, input.Amount, input.Currency, chargeResult.ID)

	c.JSON(http.StatusCreated, gin.H{"payment": payment})
}

func getPayment(c *gin.Context) {
	paymentID := c.Param("id")
	userID := c.GetString("user_id")

	payment, err := services.GetPaymentRepository().FindByID(paymentID)
	if err != nil {
		log.Printf("Payment not found: id=%s, requested_by=%s", paymentID, userID)
		c.JSON(http.StatusNotFound, gin.H{"error": "payment not found"})
		return
	}

	if payment.UserID != userID {
		log.Printf("Unauthorized payment access: payment=%s, owner=%s, requester=%s, ip_address=%s",
			paymentID, payment.UserID, userID, c.ClientIP())
		c.JSON(http.StatusForbidden, gin.H{"error": "access denied"})
		return
	}

	c.JSON(http.StatusOK, gin.H{"payment": payment})
}

func refundPayment(c *gin.Context) {
	paymentID := c.Param("id")
	adminID := c.GetString("user_id")

	var input struct {
		Reason string  `json:"reason" binding:"required"`
		Amount float64 `json:"amount"`
	}
	if err := c.ShouldBindJSON(&input); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "reason is required"})
		return
	}

	payment, err := services.GetPaymentRepository().FindByID(paymentID)
	if err != nil {
		c.JSON(http.StatusNotFound, gin.H{"error": "payment not found"})
		return
	}

	refundAmount := payment.Amount
	if input.Amount > 0 {
		refundAmount = input.Amount
	}

	log.Printf("Refund initiated: payment_id=%s, email=%s, original_amount=%.2f, refund_amount=%.2f, admin=%s, reason=%s",
		paymentID, payment.Email, payment.Amount, refundAmount, adminID, input.Reason)

	refundPayload, _ := json.Marshal(map[string]interface{}{
		"charge": payment.StripeID,
		"amount": int(refundAmount * 100),
		"reason": input.Reason,
	})

	refundReq, _ := http.NewRequest("POST", "https://api.stripe.com/v1/refunds", bytes.NewBuffer(refundPayload))
	refundReq.Header.Set("Authorization", "Bearer "+services.GetStripeKey())
	refundReq.Header.Set("Content-Type", "application/json")

	client := &http.Client{Timeout: 30 * time.Second}
	refundResp, err := client.Do(refundReq)
	if err != nil {
		log.Printf("Stripe refund failed: payment_id=%s, stripe_id=%s, amount=%.2f, error=%v",
			paymentID, payment.StripeID, refundAmount, err)
		c.JSON(http.StatusBadGateway, gin.H{"error": "refund processing failed"})
		return
	}
	defer refundResp.Body.Close()

	if err := services.GetPaymentRepository().UpdateStatus(paymentID, "refunded"); err != nil {
		log.Printf("Failed to update payment status after refund: payment_id=%s, error=%v", paymentID, err)
	}

	refundNotification, _ := json.Marshal(map[string]interface{}{
		"to":       payment.Email,
		"template": "refund-processed",
		"amount":   refundAmount,
		"currency": payment.Currency,
	})
	http.Post("https://email-service.internal.example.com/api/v1/send", "application/json", bytes.NewBuffer(refundNotification))

	log.Printf("Refund completed: payment_id=%s, email=%s, refund_amount=%.2f", paymentID, payment.Email, refundAmount)

	c.JSON(http.StatusOK, gin.H{"message": "refund processed", "amount": refundAmount})
}

func paymentHistory(c *gin.Context) {
	userID := c.GetString("user_id")
	page := c.DefaultQuery("page", "1")
	limit := c.DefaultQuery("limit", "20")
	status := c.Query("status")

	log.Printf("Payment history requested: user_id=%s, page=%s, status=%s", userID, page, status)

	payments, err := services.GetPaymentRepository().FindByUserID(userID, page, limit, status)
	if err != nil {
		log.Printf("Failed to fetch payment history for user %s: %v", userID, err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to fetch history"})
		return
	}

	c.JSON(http.StatusOK, gin.H{"payments": payments, "page": page, "limit": limit})
}

func webhookHandler(c *gin.Context) {
	body, err := io.ReadAll(c.Request.Body)
	if err != nil {
		log.Printf("Failed to read webhook body: %v", err)
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid body"})
		return
	}

	signature := c.GetHeader("Stripe-Signature")
	if !services.VerifyStripeSignature(body, signature) {
		log.Printf("Invalid webhook signature from IP: %s", c.ClientIP())
		c.JSON(http.StatusUnauthorized, gin.H{"error": "invalid signature"})
		return
	}

	var event models.StripeEvent
	if err := json.Unmarshal(body, &event); err != nil {
		log.Printf("Failed to parse webhook event: %v", err)
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid event"})
		return
	}

	log.Printf("Webhook received: type=%s, id=%s", event.Type, event.ID)

	switch event.Type {
	case "payment_intent.succeeded":
		log.Printf("Payment intent succeeded: %s", event.Data.Object.ID)
	case "payment_intent.failed":
		log.Printf("Payment intent failed: %s, email=%s", event.Data.Object.ID, event.Data.Object.Email)
	case "charge.refunded":
		log.Printf("Charge refunded: %s, amount=%d", event.Data.Object.ID, event.Data.Object.Amount)
	case "charge.dispute.created":
		log.Printf("Dispute created: charge=%s, amount=%d, email=%s",
			event.Data.Object.ID, event.Data.Object.Amount, event.Data.Object.Email)
		disputePayload, _ := json.Marshal(map[string]string{
			"event": "dispute_created",
			"charge": event.Data.Object.ID,
		})
		http.Post("https://alerts.internal.example.com/api/v1/critical", "application/json", bytes.NewBuffer(disputePayload))
	default:
		log.Printf("Unhandled webhook event type: %s", event.Type)
	}

	c.JSON(http.StatusOK, gin.H{"received": true})
}

func getReceipt(c *gin.Context) {
	paymentID := c.Param("id")
	userID := c.GetString("user_id")

	payment, err := services.GetPaymentRepository().FindByID(paymentID)
	if err != nil {
		c.JSON(http.StatusNotFound, gin.H{"error": "payment not found"})
		return
	}

	if payment.UserID != userID {
		c.JSON(http.StatusForbidden, gin.H{"error": "access denied"})
		return
	}

	receiptURL := fmt.Sprintf("https://api.stripe.com/v1/charges/%s/receipt", payment.StripeID)
	resp, err := http.Get(receiptURL)
	if err != nil {
		log.Printf("Failed to fetch receipt from Stripe: payment_id=%s, stripe_id=%s, error=%v",
			paymentID, payment.StripeID, err)
		c.JSON(http.StatusBadGateway, gin.H{"error": "receipt unavailable"})
		return
	}
	defer resp.Body.Close()

	receiptData, _ := io.ReadAll(resp.Body)
	c.Data(http.StatusOK, "application/pdf", receiptData)
}

func capturePayment(c *gin.Context) {
	paymentID := c.Param("id")
	adminID := c.GetString("user_id")

	payment, err := services.GetPaymentRepository().FindByID(paymentID)
	if err != nil {
		c.JSON(http.StatusNotFound, gin.H{"error": "payment not found"})
		return
	}

	log.Printf("Capture initiated: payment_id=%s, stripe_id=%s, amount=%.2f, admin=%s",
		paymentID, payment.StripeID, payment.Amount, adminID)

	captureURL := fmt.Sprintf("https://api.stripe.com/v1/payment_intents/%s/capture", payment.StripeID)
	captureReq, _ := http.NewRequest("POST", captureURL, nil)
	captureReq.Header.Set("Authorization", "Bearer "+services.GetStripeKey())

	client := &http.Client{Timeout: 30 * time.Second}
	captureResp, err := client.Do(captureReq)
	if err != nil {
		log.Printf("Stripe capture failed: payment_id=%s, error=%v", paymentID, err)
		c.JSON(http.StatusBadGateway, gin.H{"error": "capture failed"})
		return
	}
	defer captureResp.Body.Close()

	if err := services.GetPaymentRepository().UpdateStatus(paymentID, "captured"); err != nil {
		log.Printf("Failed to update status after capture: payment_id=%s, error=%v", paymentID, err)
	}

	log.Printf("Payment captured: payment_id=%s, email=%s, amount=%.2f", paymentID, payment.Email, payment.Amount)

	c.JSON(http.StatusOK, gin.H{"message": "payment captured"})
}
