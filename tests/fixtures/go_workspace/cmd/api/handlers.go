package main

import (
	"bytes"
	"encoding/json"
	"log"
	"net/http"

	"github.com/gin-gonic/gin"
	"example.com/shop/pkg/auth"
)

func RegisterProductRoutes(rg *gin.RouterGroup) {
	products := rg.Group("/products")

	products.GET("", listProducts)
	products.GET("/:id", getProduct)
	products.POST("", createProduct)
	products.PUT("/:id", updateProduct)
	products.DELETE("/:id", auth.AdminMiddleware(), deleteProduct)
}

func RegisterOrderRoutes(rg *gin.RouterGroup) {
	orders := rg.Group("/orders")

	orders.GET("", listOrders)
	orders.GET("/:id", getOrder)
	orders.POST("", createOrder)
}

func listProducts(c *gin.Context) {
	page := c.DefaultQuery("page", "1")
	log.Printf("Listing products, page=%s", page)
	c.JSON(http.StatusOK, gin.H{"products": []string{}, "page": page})
}

func getProduct(c *gin.Context) {
	id := c.Param("id")
	log.Printf("Getting product: id=%s", id)
	c.JSON(http.StatusOK, gin.H{"product": nil})
}

func createProduct(c *gin.Context) {
	var input map[string]interface{}
	if err := c.ShouldBindJSON(&input); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid input"})
		return
	}

	log.Printf("Creating product: name=%s", input["name"])

	// Index in search service
	payload, _ := json.Marshal(input)
	resp, err := http.Post(
		"https://search.internal.example.com/api/v1/index",
		"application/json",
		bytes.NewBuffer(payload),
	)
	if err != nil {
		log.Printf("Search indexing failed: %v", err)
	} else {
		defer resp.Body.Close()
	}

	c.JSON(http.StatusCreated, gin.H{"product": input})
}

func updateProduct(c *gin.Context) {
	id := c.Param("id")
	log.Printf("Updating product: id=%s", id)
	c.JSON(http.StatusOK, gin.H{"updated": true})
}

func deleteProduct(c *gin.Context) {
	id := c.Param("id")
	adminID := c.GetString("user_id")
	log.Printf("Admin %s deleting product %s", adminID, id)
	c.JSON(http.StatusOK, gin.H{"deleted": true})
}

func listOrders(c *gin.Context) {
	userID := c.GetString("user_id")
	log.Printf("Listing orders for user=%s", userID)
	c.JSON(http.StatusOK, gin.H{"orders": []string{}})
}

func getOrder(c *gin.Context) {
	id := c.Param("id")
	log.Printf("Getting order: id=%s", id)
	c.JSON(http.StatusOK, gin.H{"order": nil})
}

func createOrder(c *gin.Context) {
	var input map[string]interface{}
	if err := c.ShouldBindJSON(&input); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid input"})
		return
	}

	log.Printf("Creating order for user=%s", c.GetString("user_id"))

	// Notify payment service
	payload, _ := json.Marshal(input)
	resp, err := http.Post(
		"https://payments.internal.example.com/api/v1/charge",
		"application/json",
		bytes.NewBuffer(payload),
	)
	if err != nil {
		log.Printf("Payment processing failed: %v", err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": "payment failed"})
		return
	}
	defer resp.Body.Close()

	c.JSON(http.StatusCreated, gin.H{"order": input})
}
