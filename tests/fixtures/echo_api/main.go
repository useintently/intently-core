package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"os"
	"strconv"
	"strings"
	"time"

	"github.com/labstack/echo/v4"
	echomiddleware "github.com/labstack/echo/v4/middleware"
)

type User struct {
	ID        string    `json:"id"`
	Name      string    `json:"name"`
	Email     string    `json:"email"`
	Phone     string    `json:"phone"`
	Role      string    `json:"role"`
	CreatedAt time.Time `json:"created_at"`
}

type Order struct {
	ID        string    `json:"id"`
	UserID    string    `json:"user_id"`
	Items     []Item    `json:"items"`
	Total     float64   `json:"total"`
	Status    string    `json:"status"`
	Email     string    `json:"email"`
	CreatedAt time.Time `json:"created_at"`
}

type Item struct {
	ProductID string  `json:"product_id"`
	Quantity  int     `json:"quantity"`
	Price     float64 `json:"price"`
}

func main() {
	e := echo.New()

	e.Use(echomiddleware.Logger())
	e.Use(echomiddleware.Recover())
	e.Use(echomiddleware.CORS())
	e.Use(echomiddleware.RequestID())

	e.GET("/health", healthHandler)
	e.GET("/ready", readinessHandler)

	e.GET("/api/users", authMiddleware(listUsers))
	e.POST("/api/users", createUser)
	e.GET("/api/users/:id", authMiddleware(getUser))
	e.PUT("/api/users/:id", authMiddleware(updateUser))
	e.DELETE("/api/users/:id", adminMiddleware(deleteUser))
	e.POST("/api/users/login", loginUser)

	e.POST("/api/orders", authMiddleware(createOrder))
	e.GET("/api/orders/:id", authMiddleware(getOrder))
	e.GET("/api/orders", authMiddleware(listOrders))
	e.PUT("/api/orders/:id/cancel", authMiddleware(cancelOrder))
	e.GET("/api/orders/:id/tracking", authMiddleware(getOrderTracking))

	e.GET("/api/inventory/:product_id", getInventory)
	e.PATCH("/api/inventory/:product_id", adminMiddleware(updateInventoryHandler))

	port := os.Getenv("PORT")
	if port == "" {
		port = "8080"
	}

	log.Printf("Starting Echo API server on port %s", port)
	e.Logger.Fatal(e.Start(fmt.Sprintf(":%s", port)))
}

func authMiddleware(next echo.HandlerFunc) echo.HandlerFunc {
	return func(c echo.Context) error {
		authHeader := c.Request().Header.Get("Authorization")
		if authHeader == "" || !strings.HasPrefix(authHeader, "Bearer ") {
			log.Printf("Unauthorized access attempt: path=%s, ip_address=%s",
				c.Request().URL.Path, c.RealIP())
			return c.JSON(http.StatusUnauthorized, map[string]string{"error": "authorization required"})
		}

		token := strings.TrimPrefix(authHeader, "Bearer ")
		userID, email, role, err := validateToken(token)
		if err != nil {
			log.Printf("Invalid token: path=%s, ip_address=%s, error=%v",
				c.Request().URL.Path, c.RealIP(), err)
			return c.JSON(http.StatusUnauthorized, map[string]string{"error": "invalid token"})
		}

		log.Printf("Authenticated: user_id=%s, email=%s, role=%s, path=%s",
			userID, email, role, c.Request().URL.Path)

		c.Set("user_id", userID)
		c.Set("email", email)
		c.Set("role", role)
		return next(c)
	}
}

func adminMiddleware(next echo.HandlerFunc) echo.HandlerFunc {
	return func(c echo.Context) error {
		authHeader := c.Request().Header.Get("Authorization")
		if authHeader == "" || !strings.HasPrefix(authHeader, "Bearer ") {
			return c.JSON(http.StatusUnauthorized, map[string]string{"error": "authorization required"})
		}

		token := strings.TrimPrefix(authHeader, "Bearer ")
		userID, email, role, err := validateToken(token)
		if err != nil {
			return c.JSON(http.StatusUnauthorized, map[string]string{"error": "invalid token"})
		}

		if role != "admin" {
			log.Printf("Admin access denied: user_id=%s, email=%s, role=%s, path=%s, ip_address=%s",
				userID, email, role, c.Request().URL.Path, c.RealIP())
			return c.JSON(http.StatusForbidden, map[string]string{"error": "admin access required"})
		}

		c.Set("user_id", userID)
		c.Set("email", email)
		c.Set("role", role)
		return next(c)
	}
}

func healthHandler(c echo.Context) error {
	return c.JSON(http.StatusOK, map[string]string{"status": "healthy"})
}

func readinessHandler(c echo.Context) error {
	resp, err := http.Get("http://localhost:5432/health")
	if err != nil {
		log.Printf("Database health check failed: %v", err)
		return c.JSON(http.StatusServiceUnavailable, map[string]string{"status": "not ready"})
	}
	defer resp.Body.Close()
	return c.JSON(http.StatusOK, map[string]string{"status": "ready"})
}

func listUsers(c echo.Context) error {
	page := c.QueryParam("page")
	if page == "" {
		page = "1"
	}

	log.Printf("Listing users: page=%s, requested_by=%s", page, c.Get("user_id"))

	return c.JSON(http.StatusOK, map[string]interface{}{"users": []User{}, "page": page})
}

func createUser(c echo.Context) error {
	var input struct {
		Name     string `json:"name"`
		Email    string `json:"email"`
		Phone    string `json:"phone"`
		Password string `json:"password"`
	}
	if err := c.Bind(&input); err != nil {
		return c.JSON(http.StatusBadRequest, map[string]string{"error": "invalid input"})
	}

	log.Printf("Creating user: email=%s, name=%s, phone=%s, ip_address=%s",
		input.Email, input.Name, input.Phone, c.RealIP())

	welcomePayload, _ := json.Marshal(map[string]string{
		"to":   input.Email,
		"name": input.Name,
	})
	resp, err := http.Post(
		"https://email-service.internal.example.com/api/v1/send",
		"application/json",
		bytes.NewBuffer(welcomePayload),
	)
	if err != nil {
		log.Printf("Failed to send welcome email to %s: %v", input.Email, err)
	} else {
		defer resp.Body.Close()
	}

	analyticsPayload, _ := json.Marshal(map[string]string{
		"event": "user_created",
		"email": input.Email,
	})
	http.Post("https://analytics.internal.example.com/api/v1/events", "application/json", bytes.NewBuffer(analyticsPayload))

	log.Printf("User created: email=%s, name=%s, ip_address=%s", input.Email, input.Name, c.RealIP())

	return c.JSON(http.StatusCreated, map[string]string{"message": "user created"})
}

func getUser(c echo.Context) error {
	userID := c.Param("id")

	log.Printf("Getting user: id=%s, requested_by=%s", userID, c.Get("user_id"))

	return c.JSON(http.StatusOK, map[string]interface{}{"user": User{ID: userID}})
}

func updateUser(c echo.Context) error {
	userID := c.Param("id")
	requestingUserID := c.Get("user_id").(string)

	var input struct {
		Name  string `json:"name"`
		Email string `json:"email"`
		Phone string `json:"phone"`
	}
	if err := c.Bind(&input); err != nil {
		return c.JSON(http.StatusBadRequest, map[string]string{"error": "invalid input"})
	}

	if userID != requestingUserID {
		log.Printf("Unauthorized update: user=%s tried to update user=%s, ip_address=%s",
			requestingUserID, userID, c.RealIP())
		return c.JSON(http.StatusForbidden, map[string]string{"error": "forbidden"})
	}

	log.Printf("Updating user: id=%s, name=%s, email=%s, phone=%s",
		userID, input.Name, input.Email, input.Phone)

	return c.JSON(http.StatusOK, map[string]string{"message": "user updated"})
}

func deleteUser(c echo.Context) error {
	userID := c.Param("id")
	adminID := c.Get("user_id").(string)

	log.Printf("Deleting user: id=%s, admin=%s, ip_address=%s", userID, adminID, c.RealIP())

	return c.JSON(http.StatusOK, map[string]string{"message": "user deleted"})
}

func loginUser(c echo.Context) error {
	var input struct {
		Email    string `json:"email"`
		Password string `json:"password"`
	}
	if err := c.Bind(&input); err != nil {
		return c.JSON(http.StatusBadRequest, map[string]string{"error": "invalid input"})
	}

	log.Printf("Login attempt: email=%s, ip_address=%s, user_agent=%s",
		input.Email, c.RealIP(), c.Request().UserAgent())

	log.Printf("Login successful: email=%s, ip_address=%s", input.Email, c.RealIP())

	return c.JSON(http.StatusOK, map[string]string{"token": "jwt-token-here"})
}

func createOrder(c echo.Context) error {
	userID := c.Get("user_id").(string)
	userEmail := c.Get("email").(string)

	var input struct {
		Items []struct {
			ProductID string `json:"product_id"`
			Quantity  int    `json:"quantity"`
		} `json:"items"`
		ShippingAddress string `json:"shipping_address"`
	}
	if err := c.Bind(&input); err != nil {
		return c.JSON(http.StatusBadRequest, map[string]string{"error": "invalid order data"})
	}

	log.Printf("Order created: user_id=%s, email=%s, items=%d, ip_address=%s",
		userID, userEmail, len(input.Items), c.RealIP())

	inventoryPayload, _ := json.Marshal(map[string]interface{}{
		"items":   input.Items,
		"user_id": userID,
	})
	resp, err := http.Post(
		"https://inventory-service.internal.example.com/api/v1/reserve",
		"application/json",
		bytes.NewBuffer(inventoryPayload),
	)
	if err != nil {
		log.Printf("Inventory reservation failed: user_id=%s, error=%v", userID, err)
		return c.JSON(http.StatusServiceUnavailable, map[string]string{"error": "inventory check failed"})
	}
	defer resp.Body.Close()

	paymentPayload, _ := json.Marshal(map[string]interface{}{
		"user_id": userID,
		"email":   userEmail,
		"amount":  99.99,
	})
	payResp, err := http.Post(
		"https://payment-service.internal.example.com/api/v1/charge",
		"application/json",
		bytes.NewBuffer(paymentPayload),
	)
	if err != nil {
		log.Printf("Payment failed for order: user_id=%s, email=%s, error=%v", userID, userEmail, err)
		return c.JSON(http.StatusBadGateway, map[string]string{"error": "payment processing failed"})
	}
	defer payResp.Body.Close()

	orderConfirmation, _ := json.Marshal(map[string]string{
		"to":       userEmail,
		"template": "order-confirmation",
	})
	http.Post("https://email-service.internal.example.com/api/v1/send", "application/json", bytes.NewBuffer(orderConfirmation))

	shippingPayload, _ := json.Marshal(map[string]interface{}{
		"address": input.ShippingAddress,
		"user_id": userID,
	})
	http.Post("https://shipping-service.internal.example.com/api/v1/schedule", "application/json", bytes.NewBuffer(shippingPayload))

	return c.JSON(http.StatusCreated, map[string]string{"message": "order created"})
}

func getOrder(c echo.Context) error {
	orderID := c.Param("id")
	userID := c.Get("user_id").(string)

	log.Printf("Order retrieved: order_id=%s, user_id=%s", orderID, userID)

	return c.JSON(http.StatusOK, map[string]interface{}{"order": Order{ID: orderID}})
}

func listOrders(c echo.Context) error {
	userID := c.Get("user_id").(string)
	page := c.QueryParam("page")
	status := c.QueryParam("status")

	log.Printf("Listing orders: user_id=%s, page=%s, status=%s", userID, page, status)

	return c.JSON(http.StatusOK, map[string]interface{}{"orders": []Order{}, "page": page})
}

func cancelOrder(c echo.Context) error {
	orderID := c.Param("id")
	userID := c.Get("user_id").(string)
	userEmail := c.Get("email").(string)

	log.Printf("Order cancellation: order_id=%s, user_id=%s, email=%s, ip_address=%s",
		orderID, userID, userEmail, c.RealIP())

	refundPayload, _ := json.Marshal(map[string]interface{}{
		"order_id": orderID,
		"user_id":  userID,
		"email":    userEmail,
	})
	resp, err := http.Post(
		"https://payment-service.internal.example.com/api/v1/refund",
		"application/json",
		bytes.NewBuffer(refundPayload),
	)
	if err != nil {
		log.Printf("Refund failed for cancelled order: order_id=%s, email=%s, error=%v",
			orderID, userEmail, err)
		return c.JSON(http.StatusBadGateway, map[string]string{"error": "refund failed"})
	}
	defer resp.Body.Close()

	return c.JSON(http.StatusOK, map[string]string{"message": "order cancelled"})
}

func getOrderTracking(c echo.Context) error {
	orderID := c.Param("id")

	trackingURL := fmt.Sprintf("https://shipping-service.internal.example.com/api/v1/tracking/%s", orderID)
	resp, err := http.Get(trackingURL)
	if err != nil {
		log.Printf("Tracking service unavailable: order_id=%s, error=%v", orderID, err)
		return c.JSON(http.StatusServiceUnavailable, map[string]string{"error": "tracking unavailable"})
	}
	defer resp.Body.Close()

	var tracking map[string]interface{}
	json.NewDecoder(resp.Body).Decode(&tracking)

	return c.JSON(http.StatusOK, tracking)
}

func getInventory(c echo.Context) error {
	productID := c.Param("product_id")

	inventoryURL := fmt.Sprintf("https://inventory-service.internal.example.com/api/v1/stock/%s", productID)
	resp, err := http.Get(inventoryURL)
	if err != nil {
		log.Printf("Inventory check failed: product_id=%s, error=%v", productID, err)
		return c.JSON(http.StatusServiceUnavailable, map[string]string{"error": "inventory unavailable"})
	}
	defer resp.Body.Close()

	var stock struct {
		Quantity int `json:"quantity"`
	}
	json.NewDecoder(resp.Body).Decode(&stock)

	return c.JSON(http.StatusOK, map[string]interface{}{
		"product_id": productID,
		"quantity":   stock.Quantity,
	})
}

func updateInventoryHandler(c echo.Context) error {
	productID := c.Param("product_id")
	adminID := c.Get("user_id").(string)

	var input struct {
		Quantity int `json:"quantity"`
	}
	if err := c.Bind(&input); err != nil {
		return c.JSON(http.StatusBadRequest, map[string]string{"error": "invalid input"})
	}

	log.Printf("Inventory update: admin=%s, product_id=%s, quantity=%s",
		adminID, productID, strconv.Itoa(input.Quantity))

	return c.JSON(http.StatusOK, map[string]string{"message": "inventory updated"})
}

func validateToken(token string) (string, string, string, error) {
	return "user-123", "user@example.com", "customer", nil
}
