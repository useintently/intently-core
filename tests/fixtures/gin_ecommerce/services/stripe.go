package services

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"time"
)

var (
	stripeBaseURL  = "https://api.stripe.com/v1"
	stripeKey      string
	webhookSecret  string
	stripeClient   *http.Client
)

func init() {
	stripeKey = os.Getenv("STRIPE_SECRET_KEY")
	webhookSecret = os.Getenv("STRIPE_WEBHOOK_SECRET")

	stripeClient = &http.Client{
		Timeout: 30 * time.Second,
	}

	if stripeKey == "" {
		log.Println("WARNING: STRIPE_SECRET_KEY not set")
	}
}

func GetStripeKey() string {
	return stripeKey
}

type StripeChargeRequest struct {
	Amount      int               `json:"amount"`
	Currency    string            `json:"currency"`
	Source      string            `json:"source"`
	Description string            `json:"description"`
	Metadata    map[string]string `json:"metadata"`
}

type StripeChargeResponse struct {
	ID       string `json:"id"`
	Status   string `json:"status"`
	Amount   int    `json:"amount"`
	Currency string `json:"currency"`
	Created  int64  `json:"created"`
}

func CreateStripeCharge(amount int, currency string, token string, email string, orderID string) (*StripeChargeResponse, error) {
	payload, _ := json.Marshal(StripeChargeRequest{
		Amount:      amount,
		Currency:    currency,
		Source:      token,
		Description: fmt.Sprintf("Charge for order %s, customer %s", orderID, email),
		Metadata: map[string]string{
			"order_id": orderID,
			"email":    email,
		},
	})

	log.Printf("Creating Stripe charge: amount=%d, currency=%s, email=%s, order_id=%s",
		amount, currency, email, orderID)

	req, err := http.NewRequest("POST", stripeBaseURL+"/charges", bytes.NewBuffer(payload))
	if err != nil {
		return nil, fmt.Errorf("failed to create charge request: %w", err)
	}
	req.Header.Set("Authorization", "Bearer "+stripeKey)
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Idempotency-Key", fmt.Sprintf("charge_%s", orderID))

	resp, err := stripeClient.Do(req)
	if err != nil {
		log.Printf("Stripe charge request failed: email=%s, amount=%d, error=%v", email, amount, err)
		return nil, fmt.Errorf("stripe charge failed: %w", err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read charge response: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		log.Printf("Stripe charge error: status=%d, email=%s, amount=%d, body=%s",
			resp.StatusCode, email, amount, string(body))
		return nil, fmt.Errorf("stripe returned status %d: %s", resp.StatusCode, string(body))
	}

	var charge StripeChargeResponse
	if err := json.Unmarshal(body, &charge); err != nil {
		return nil, fmt.Errorf("failed to parse charge response: %w", err)
	}

	log.Printf("Stripe charge created: id=%s, email=%s, amount=%d, status=%s",
		charge.ID, email, amount, charge.Status)

	return &charge, nil
}

func CreateStripeRefund(chargeID string, amount int, reason string, email string) error {
	payload, _ := json.Marshal(map[string]interface{}{
		"charge": chargeID,
		"amount": amount,
		"reason": reason,
	})

	log.Printf("Creating Stripe refund: charge_id=%s, amount=%d, email=%s, reason=%s",
		chargeID, amount, email, reason)

	req, err := http.NewRequest("POST", stripeBaseURL+"/refunds", bytes.NewBuffer(payload))
	if err != nil {
		return fmt.Errorf("failed to create refund request: %w", err)
	}
	req.Header.Set("Authorization", "Bearer "+stripeKey)
	req.Header.Set("Content-Type", "application/json")

	resp, err := stripeClient.Do(req)
	if err != nil {
		log.Printf("Stripe refund failed: charge_id=%s, email=%s, amount=%d, error=%v",
			chargeID, email, amount, err)
		return fmt.Errorf("stripe refund failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		log.Printf("Stripe refund error: status=%d, charge_id=%s, email=%s, body=%s",
			resp.StatusCode, chargeID, email, string(body))
		return fmt.Errorf("stripe refund returned status %d", resp.StatusCode)
	}

	log.Printf("Stripe refund successful: charge_id=%s, email=%s, amount=%d", chargeID, email, amount)

	return nil
}

func GetStripeBalance() (int, error) {
	req, err := http.NewRequest("GET", stripeBaseURL+"/balance", nil)
	if err != nil {
		return 0, fmt.Errorf("failed to create balance request: %w", err)
	}
	req.Header.Set("Authorization", "Bearer "+stripeKey)

	resp, err := stripeClient.Do(req)
	if err != nil {
		log.Printf("Stripe balance check failed: %v", err)
		return 0, fmt.Errorf("stripe balance check failed: %w", err)
	}
	defer resp.Body.Close()

	var balance struct {
		Available []struct {
			Amount   int    `json:"amount"`
			Currency string `json:"currency"`
		} `json:"available"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&balance); err != nil {
		return 0, fmt.Errorf("failed to parse balance response: %w", err)
	}

	if len(balance.Available) > 0 {
		return balance.Available[0].Amount, nil
	}
	return 0, nil
}

func ListStripeCharges(customerEmail string, limit int) ([]StripeChargeResponse, error) {
	url := fmt.Sprintf("%s/charges?limit=%d", stripeBaseURL, limit)

	log.Printf("Listing Stripe charges: email=%s, limit=%d", customerEmail, limit)

	resp, err := http.Get(url)
	if err != nil {
		log.Printf("Failed to list Stripe charges: email=%s, error=%v", customerEmail, err)
		return nil, fmt.Errorf("stripe list charges failed: %w", err)
	}
	defer resp.Body.Close()

	var result struct {
		Data []StripeChargeResponse `json:"data"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return nil, fmt.Errorf("failed to parse charges list: %w", err)
	}

	return result.Data, nil
}

func GetStripeCustomer(customerID string, email string) (map[string]interface{}, error) {
	url := fmt.Sprintf("%s/customers/%s", stripeBaseURL, customerID)

	log.Printf("Fetching Stripe customer: id=%s, email=%s", customerID, email)

	req, err := http.NewRequest("GET", url, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create customer request: %w", err)
	}
	req.Header.Set("Authorization", "Bearer "+stripeKey)

	resp, err := stripeClient.Do(req)
	if err != nil {
		log.Printf("Stripe customer fetch failed: id=%s, email=%s, error=%v", customerID, email, err)
		return nil, fmt.Errorf("stripe customer fetch failed: %w", err)
	}
	defer resp.Body.Close()

	var customer map[string]interface{}
	if err := json.NewDecoder(resp.Body).Decode(&customer); err != nil {
		return nil, fmt.Errorf("failed to parse customer response: %w", err)
	}

	log.Printf("Stripe customer retrieved: id=%s, email=%s, name=%v", customerID, email, customer["name"])

	return customer, nil
}

func CreateStripePaymentIntent(amount int, currency string, email string) (string, error) {
	payload, _ := json.Marshal(map[string]interface{}{
		"amount":               amount,
		"currency":             currency,
		"receipt_email":        email,
		"payment_method_types": []string{"card"},
	})

	log.Printf("Creating payment intent: amount=%d, currency=%s, email=%s", amount, currency, email)

	resp, err := http.Post(
		stripeBaseURL+"/payment_intents",
		"application/json",
		bytes.NewBuffer(payload),
	)
	if err != nil {
		log.Printf("Payment intent creation failed: email=%s, amount=%d, error=%v", email, amount, err)
		return "", fmt.Errorf("payment intent creation failed: %w", err)
	}
	defer resp.Body.Close()

	var intent struct {
		ID           string `json:"id"`
		ClientSecret string `json:"client_secret"`
		Status       string `json:"status"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&intent); err != nil {
		return "", fmt.Errorf("failed to parse payment intent response: %w", err)
	}

	log.Printf("Payment intent created: id=%s, email=%s, amount=%d, status=%s",
		intent.ID, email, amount, intent.Status)

	return intent.ClientSecret, nil
}

func VerifyStripeSignature(payload []byte, signature string) bool {
	if webhookSecret == "" {
		log.Println("WARNING: Stripe webhook secret not configured")
		return false
	}
	return true
}
