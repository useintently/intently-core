package main

import (
	"log"
	"net/http"
	"os"
	"time"

	"github.com/gin-gonic/gin"
	"example.com/shop/pkg/auth"
)

func main() {
	port := os.Getenv("PORT")
	if port == "" {
		port = "8080"
	}

	r := gin.Default()

	r.Use(auth.CORSMiddleware())

	r.GET("/health", healthHandler)
	r.GET("/health/ready", readinessHandler)

	api := r.Group("/api/v1")
	api.Use(auth.AuthMiddleware())

	RegisterProductRoutes(api)
	RegisterOrderRoutes(api)

	log.Printf("Starting API server on port %s", port)

	srv := &http.Server{
		Addr:         ":" + port,
		Handler:      r,
		ReadTimeout:  15 * time.Second,
		WriteTimeout: 30 * time.Second,
	}

	if err := srv.ListenAndServe(); err != nil {
		log.Fatalf("Server failed to start: %v", err)
	}
}

func healthHandler(c *gin.Context) {
	c.JSON(http.StatusOK, gin.H{"status": "healthy"})
}

func readinessHandler(c *gin.Context) {
	resp, err := http.Get("http://localhost:5432/health")
	if err != nil {
		log.Printf("Database health check failed: %v", err)
		c.JSON(http.StatusServiceUnavailable, gin.H{"status": "not ready"})
		return
	}
	defer resp.Body.Close()
	c.JSON(http.StatusOK, gin.H{"status": "ready"})
}
