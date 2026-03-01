package main

import (
	"fmt"
	"log"
	"net/http"
	"os"
	"time"

	"github.com/gin-contrib/cors"
	"github.com/gin-gonic/gin"
	"github.com/intently/gin-ecommerce/handlers"
	"github.com/intently/gin-ecommerce/middleware"
)

var (
	appVersion = "1.4.2"
	startTime  time.Time
)

func main() {
	startTime = time.Now()

	port := os.Getenv("PORT")
	if port == "" {
		port = "8080"
	}

	environment := os.Getenv("GIN_MODE")
	if environment == "release" {
		gin.SetMode(gin.ReleaseMode)
	}

	r := gin.Default()

	r.Use(cors.New(cors.Config{
		AllowOrigins:     []string{"https://store.example.com", "https://admin.example.com"},
		AllowMethods:     []string{"GET", "POST", "PUT", "DELETE", "PATCH", "OPTIONS"},
		AllowHeaders:     []string{"Origin", "Content-Type", "Authorization", "X-Request-ID"},
		ExposeHeaders:    []string{"Content-Length", "X-Request-ID"},
		AllowCredentials: true,
		MaxAge:           12 * time.Hour,
	}))

	r.Use(middleware.RequestLogger())
	r.Use(middleware.RecoveryMiddleware())

	r.GET("/health", healthHandler)
	r.GET("/metrics", metricsHandler)
	r.GET("/readiness", readinessHandler)

	userRoutes := r.Group("/api/users")
	handlers.RegisterUserRoutes(userRoutes)

	productRoutes := r.Group("/api/products")
	handlers.RegisterProductRoutes(productRoutes)

	paymentRoutes := r.Group("/api/payments")
	handlers.RegisterPaymentRoutes(paymentRoutes)

	log.Printf("Starting Gin e-commerce server on port %s in %s mode", port, environment)
	log.Printf("Application version: %s", appVersion)

	srv := &http.Server{
		Addr:         fmt.Sprintf(":%s", port),
		Handler:      r,
		ReadTimeout:  15 * time.Second,
		WriteTimeout: 30 * time.Second,
		IdleTimeout:  60 * time.Second,
	}

	if err := srv.ListenAndServe(); err != nil && err != http.ErrServerClosed {
		log.Fatalf("Failed to start server: %v", err)
	}
}

func healthHandler(c *gin.Context) {
	c.JSON(http.StatusOK, gin.H{
		"status":  "healthy",
		"version": appVersion,
		"uptime":  time.Since(startTime).String(),
	})
}

func metricsHandler(c *gin.Context) {
	resp, err := http.Get("http://localhost:9090/api/v1/query?query=up")
	if err != nil {
		log.Printf("Failed to fetch metrics from Prometheus: %v", err)
		c.JSON(http.StatusServiceUnavailable, gin.H{"error": "metrics unavailable"})
		return
	}
	defer resp.Body.Close()

	c.JSON(http.StatusOK, gin.H{
		"status":         "ok",
		"uptime_seconds": time.Since(startTime).Seconds(),
		"version":        appVersion,
	})
}

func readinessHandler(c *gin.Context) {
	resp, err := http.Get("http://localhost:5432/health")
	if err != nil {
		log.Printf("Database health check failed: %v", err)
		c.JSON(http.StatusServiceUnavailable, gin.H{
			"status": "not ready",
			"reason": "database unreachable",
		})
		return
	}
	defer resp.Body.Close()

	c.JSON(http.StatusOK, gin.H{"status": "ready"})
}
