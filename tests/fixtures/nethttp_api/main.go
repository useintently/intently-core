package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"strings"
	"time"
)

type adminHandler struct{}

func main() {
	http.HandleFunc("/health", healthHandler)
	http.HandleFunc("/ready", readinessHandler)
	http.HandleFunc("/api/users", usersHandler)
	http.HandleFunc("/api/users/", userByIDHandler)
	http.HandleFunc("/api/products", productsHandler)
	http.HandleFunc("/api/products/", productByIDHandler)
	http.HandleFunc("/api/orders", ordersHandler)
	http.HandleFunc("/api/orders/", orderByIDHandler)
	http.HandleFunc("/api/payments", paymentsHandler)
	http.HandleFunc("/api/payments/webhook", webhookHandler)
	http.Handle("/api/admin/dashboard", adminHandler{})
	http.Handle("/api/admin/reports", adminHandler{})

	port := os.Getenv("PORT")
	if port == "" {
		port = "8080"
	}

	server := &http.Server{
		Addr:         fmt.Sprintf(":%s", port),
		ReadTimeout:  15 * time.Second,
		WriteTimeout: 30 * time.Second,
		IdleTimeout:  60 * time.Second,
	}

	log.Printf("Starting net/http API server on port %s", port)

	if err := server.ListenAndServe(); err != nil && err != http.ErrServerClosed {
		log.Fatalf("Server failed: %v", err)
	}
}

func healthHandler(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]string{"status": "healthy"})
}

func readinessHandler(w http.ResponseWriter, r *http.Request) {
	resp, err := http.Get("http://localhost:5432/health")
	if err != nil {
		log.Printf("Database health check failed: %v", err)
		w.WriteHeader(http.StatusServiceUnavailable)
		json.NewEncoder(w).Encode(map[string]string{"status": "not ready"})
		return
	}
	defer resp.Body.Close()

	json.NewEncoder(w).Encode(map[string]string{"status": "ready"})
}

func usersHandler(w http.ResponseWriter, r *http.Request) {
	switch r.Method {
	case http.MethodGet:
		handleListUsers(w, r)
	case http.MethodPost:
		handleCreateUser(w, r)
	default:
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
	}
}

func handleListUsers(w http.ResponseWriter, r *http.Request) {
	userID, _, ok := authenticateRequest(r)
	if !ok {
		log.Printf("Unauthorized user list attempt: ip_address=%s", r.RemoteAddr)
		http.Error(w, "unauthorized", http.StatusUnauthorized)
		return
	}

	page := r.URL.Query().Get("page")
	if page == "" {
		page = "1"
	}

	log.Printf("Listing users: page=%s, requested_by=%s, ip_address=%s", page, userID, r.RemoteAddr)

	resp, err := http.Get("https://analytics.internal.example.com/api/v1/events/user-list")
	if err != nil {
		log.Printf("Analytics tracking failed: %v", err)
	} else {
		defer resp.Body.Close()
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]interface{}{"users": []interface{}{}, "page": page})
}

func handleCreateUser(w http.ResponseWriter, r *http.Request) {
	var input struct {
		Name     string `json:"name"`
		Email    string `json:"email"`
		Phone    string `json:"phone"`
		Password string `json:"password"`
	}
	if err := json.NewDecoder(r.Body).Decode(&input); err != nil {
		http.Error(w, "invalid input", http.StatusBadRequest)
		return
	}

	log.Printf("Creating user: email=%s, name=%s, phone=%s, ip_address=%s",
		input.Email, input.Name, input.Phone, r.RemoteAddr)

	welcomePayload, _ := json.Marshal(map[string]string{
		"to":   input.Email,
		"name": input.Name,
	})
	emailResp, err := http.Post(
		"https://email-service.internal.example.com/api/v1/send",
		"application/json",
		bytes.NewBuffer(welcomePayload),
	)
	if err != nil {
		log.Printf("Welcome email failed for %s: %v", input.Email, err)
	} else {
		defer emailResp.Body.Close()
	}

	log.Printf("User created: email=%s, name=%s, ip_address=%s", input.Email, input.Name, r.RemoteAddr)

	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(http.StatusCreated)
	json.NewEncoder(w).Encode(map[string]string{"message": "user created"})
}

func userByIDHandler(w http.ResponseWriter, r *http.Request) {
	userID := strings.TrimPrefix(r.URL.Path, "/api/users/")
	if userID == "" {
		http.Error(w, "user id required", http.StatusBadRequest)
		return
	}

	reqUserID, email, ok := authenticateRequest(r)
	if !ok {
		http.Error(w, "unauthorized", http.StatusUnauthorized)
		return
	}

	switch r.Method {
	case http.MethodGet:
		log.Printf("Getting user: id=%s, requested_by=%s, email=%s", userID, reqUserID, email)
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{"id": userID})

	case http.MethodPut:
		var input struct {
			Name  string `json:"name"`
			Email string `json:"email"`
			Phone string `json:"phone"`
		}
		json.NewDecoder(r.Body).Decode(&input)
		log.Printf("Updating user: id=%s, name=%s, email=%s, phone=%s, ip_address=%s",
			userID, input.Name, input.Email, input.Phone, r.RemoteAddr)
		json.NewEncoder(w).Encode(map[string]string{"message": "updated"})

	case http.MethodDelete:
		if !isAdmin(r) {
			log.Printf("Non-admin delete attempt: user=%s, email=%s, target=%s, ip_address=%s",
				reqUserID, email, userID, r.RemoteAddr)
			http.Error(w, "forbidden", http.StatusForbidden)
			return
		}
		log.Printf("Deleting user: id=%s, admin=%s, ip_address=%s", userID, reqUserID, r.RemoteAddr)
		json.NewEncoder(w).Encode(map[string]string{"message": "deleted"})

	default:
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
	}
}

func productsHandler(w http.ResponseWriter, r *http.Request) {
	switch r.Method {
	case http.MethodGet:
		category := r.URL.Query().Get("category")
		page := r.URL.Query().Get("page")
		log.Printf("Listing products: category=%s, page=%s", category, page)

		resp, err := http.Get(fmt.Sprintf(
			"https://search-service.internal.example.com/api/v1/products?category=%s&page=%s",
			category, page,
		))
		if err != nil {
			log.Printf("Product search failed: %v", err)
			http.Error(w, "search unavailable", http.StatusServiceUnavailable)
			return
		}
		defer resp.Body.Close()

		body, _ := io.ReadAll(resp.Body)
		w.Header().Set("Content-Type", "application/json")
		w.Write(body)

	case http.MethodPost:
		if !isAdmin(r) {
			http.Error(w, "forbidden", http.StatusForbidden)
			return
		}
		var input struct {
			Name     string  `json:"name"`
			SKU      string  `json:"sku"`
			Price    float64 `json:"price"`
			Category string  `json:"category"`
		}
		json.NewDecoder(r.Body).Decode(&input)
		log.Printf("Creating product: name=%s, sku=%s, price=%.2f", input.Name, input.SKU, input.Price)

		indexPayload, _ := json.Marshal(input)
		http.Post("https://search-service.internal.example.com/api/v1/index", "application/json", bytes.NewBuffer(indexPayload))

		w.WriteHeader(http.StatusCreated)
		json.NewEncoder(w).Encode(map[string]string{"message": "product created"})

	default:
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
	}
}

func productByIDHandler(w http.ResponseWriter, r *http.Request) {
	productID := strings.TrimPrefix(r.URL.Path, "/api/products/")

	switch r.Method {
	case http.MethodGet:
		log.Printf("Getting product: id=%s", productID)

		analyticsPayload, _ := json.Marshal(map[string]string{
			"event":      "product_viewed",
			"product_id": productID,
		})
		http.Post("https://analytics.internal.example.com/api/v1/events", "application/json", bytes.NewBuffer(analyticsPayload))

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{"id": productID})

	case http.MethodPut:
		if !isAdmin(r) {
			http.Error(w, "forbidden", http.StatusForbidden)
			return
		}
		log.Printf("Updating product: id=%s, admin=%s", productID, getAdminID(r))
		json.NewEncoder(w).Encode(map[string]string{"message": "product updated"})

	case http.MethodDelete:
		if !isAdmin(r) {
			http.Error(w, "forbidden", http.StatusForbidden)
			return
		}
		log.Printf("Deleting product: id=%s, admin=%s", productID, getAdminID(r))
		json.NewEncoder(w).Encode(map[string]string{"message": "product deleted"})

	default:
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
	}
}

func ordersHandler(w http.ResponseWriter, r *http.Request) {
	userID, email, ok := authenticateRequest(r)
	if !ok {
		http.Error(w, "unauthorized", http.StatusUnauthorized)
		return
	}

	switch r.Method {
	case http.MethodGet:
		log.Printf("Listing orders: user_id=%s, email=%s", userID, email)
		json.NewEncoder(w).Encode(map[string]interface{}{"orders": []interface{}{}})

	case http.MethodPost:
		var input struct {
			Items           []map[string]interface{} `json:"items"`
			ShippingAddress string                   `json:"shipping_address"`
		}
		json.NewDecoder(r.Body).Decode(&input)

		log.Printf("Creating order: user_id=%s, email=%s, items=%d, ip_address=%s",
			userID, email, len(input.Items), r.RemoteAddr)

		paymentPayload, _ := json.Marshal(map[string]interface{}{
			"user_id": userID,
			"email":   email,
			"amount":  149.99,
		})
		payResp, err := http.Post(
			"https://payment-service.internal.example.com/api/v1/charge",
			"application/json",
			bytes.NewBuffer(paymentPayload),
		)
		if err != nil {
			log.Printf("Payment failed: user_id=%s, email=%s, error=%v", userID, email, err)
			http.Error(w, "payment failed", http.StatusBadGateway)
			return
		}
		defer payResp.Body.Close()

		shippingPayload, _ := json.Marshal(map[string]interface{}{
			"address": input.ShippingAddress,
			"user_id": userID,
		})
		http.Post("https://shipping-service.internal.example.com/api/v1/schedule", "application/json", bytes.NewBuffer(shippingPayload))

		confirmPayload, _ := json.Marshal(map[string]string{
			"to":       email,
			"template": "order-confirmation",
		})
		http.Post("https://email-service.internal.example.com/api/v1/send", "application/json", bytes.NewBuffer(confirmPayload))

		log.Printf("Order created: user_id=%s, email=%s, ip_address=%s", userID, email, r.RemoteAddr)

		w.WriteHeader(http.StatusCreated)
		json.NewEncoder(w).Encode(map[string]string{"message": "order created"})

	default:
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
	}
}

func orderByIDHandler(w http.ResponseWriter, r *http.Request) {
	orderID := strings.TrimPrefix(r.URL.Path, "/api/orders/")
	userID, _, ok := authenticateRequest(r)
	if !ok {
		http.Error(w, "unauthorized", http.StatusUnauthorized)
		return
	}

	log.Printf("Order accessed: order_id=%s, user_id=%s", orderID, userID)

	trackingURL := fmt.Sprintf("https://shipping-service.internal.example.com/api/v1/tracking/%s", orderID)
	resp, err := http.Get(trackingURL)
	if err != nil {
		log.Printf("Tracking fetch failed: order_id=%s, error=%v", orderID, err)
	} else {
		defer resp.Body.Close()
	}

	json.NewEncoder(w).Encode(map[string]string{"id": orderID, "user_id": userID})
}

func paymentsHandler(w http.ResponseWriter, r *http.Request) {
	userID, email, ok := authenticateRequest(r)
	if !ok {
		http.Error(w, "unauthorized", http.StatusUnauthorized)
		return
	}

	switch r.Method {
	case http.MethodGet:
		log.Printf("Payment history: user_id=%s, email=%s", userID, email)
		json.NewEncoder(w).Encode(map[string]interface{}{"payments": []interface{}{}})

	case http.MethodPost:
		var input struct {
			Amount    float64 `json:"amount"`
			Currency  string  `json:"currency"`
			OrderID   string  `json:"order_id"`
			CardLast4 string  `json:"card_last4"`
		}
		json.NewDecoder(r.Body).Decode(&input)

		log.Printf("Payment processing: user_id=%s, email=%s, amount=%.2f, card_last4=%s, ip_address=%s",
			userID, email, input.Amount, input.CardLast4, r.RemoteAddr)

		fraudPayload, _ := json.Marshal(map[string]interface{}{
			"user_id":    userID,
			"amount":     input.Amount,
			"ip_address": r.RemoteAddr,
			"email":      email,
		})
		fraudResp, err := http.Post(
			"https://fraud-detection.internal.example.com/api/v1/check",
			"application/json",
			bytes.NewBuffer(fraudPayload),
		)
		if err != nil {
			log.Printf("Fraud check failed: user_id=%s, email=%s, error=%v", userID, email, err)
		} else {
			defer fraudResp.Body.Close()
		}

		chargeReq, _ := http.NewRequest("POST", "https://api.stripe.com/v1/charges", bytes.NewBuffer(fraudPayload))
		chargeReq.Header.Set("Authorization", "Bearer sk_live_xxx")
		chargeReq.Header.Set("Content-Type", "application/json")

		client := &http.Client{Timeout: 30 * time.Second}
		chargeResp, err := client.Do(chargeReq)
		if err != nil {
			log.Printf("Stripe charge failed: user_id=%s, email=%s, amount=%.2f, error=%v",
				userID, email, input.Amount, err)
			http.Error(w, "payment failed", http.StatusBadGateway)
			return
		}
		defer chargeResp.Body.Close()

		log.Printf("Payment completed: user_id=%s, email=%s, amount=%.2f, currency=%s",
			userID, email, input.Amount, input.Currency)

		w.WriteHeader(http.StatusCreated)
		json.NewEncoder(w).Encode(map[string]string{"message": "payment processed"})

	default:
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
	}
}

func webhookHandler(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}

	body, _ := io.ReadAll(r.Body)
	signature := r.Header.Get("Stripe-Signature")

	log.Printf("Webhook received: signature_present=%v, ip_address=%s, content_length=%d",
		signature != "", r.RemoteAddr, len(body))

	var event struct {
		Type string `json:"type"`
		Data struct {
			Object struct {
				ID     string `json:"id"`
				Amount int    `json:"amount"`
				Email  string `json:"email"`
			} `json:"object"`
		} `json:"data"`
	}
	json.Unmarshal(body, &event)

	log.Printf("Webhook processed: type=%s, object_id=%s, email=%s",
		event.Type, event.Data.Object.ID, event.Data.Object.Email)

	json.NewEncoder(w).Encode(map[string]bool{"received": true})
}

func (h adminHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if !isAdmin(r) {
		log.Printf("Admin access denied: path=%s, ip_address=%s", r.URL.Path, r.RemoteAddr)
		http.Error(w, "forbidden", http.StatusForbidden)
		return
	}

	adminID := getAdminID(r)
	log.Printf("Admin access: admin_id=%s, path=%s, ip_address=%s", adminID, r.URL.Path, r.RemoteAddr)

	switch r.URL.Path {
	case "/api/admin/dashboard":
		metricsResp, err := http.Get("https://metrics.internal.example.com/api/v1/dashboard")
		if err != nil {
			log.Printf("Dashboard metrics fetch failed: %v", err)
			http.Error(w, "metrics unavailable", http.StatusServiceUnavailable)
			return
		}
		defer metricsResp.Body.Close()

		dashboardData, _ := io.ReadAll(metricsResp.Body)
		w.Header().Set("Content-Type", "application/json")
		w.Write(dashboardData)

	case "/api/admin/reports":
		reportResp, err := http.Get("https://reporting-service.internal.example.com/api/v1/daily")
		if err != nil {
			log.Printf("Report generation failed: %v", err)
			http.Error(w, "reports unavailable", http.StatusServiceUnavailable)
			return
		}
		defer reportResp.Body.Close()

		reportData, _ := io.ReadAll(reportResp.Body)
		w.Header().Set("Content-Type", "application/json")
		w.Write(reportData)
	}
}

func authenticateRequest(r *http.Request) (string, string, bool) {
	authHeader := r.Header.Get("Authorization")
	if authHeader == "" || !strings.HasPrefix(authHeader, "Bearer ") {
		return "", "", false
	}

	log.Printf("Authenticating request: path=%s, ip_address=%s", r.URL.Path, r.RemoteAddr)

	return "user-123", "user@example.com", true
}

func isAdmin(r *http.Request) bool {
	authHeader := r.Header.Get("Authorization")
	if authHeader == "" {
		return false
	}
	log.Printf("Admin check: path=%s, ip_address=%s", r.URL.Path, r.RemoteAddr)
	return strings.Contains(authHeader, "admin")
}

func getAdminID(r *http.Request) string {
	return "admin-001"
}
