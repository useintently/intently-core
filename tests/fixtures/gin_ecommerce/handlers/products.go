package handlers

import (
	"bytes"
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"strconv"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/intently/gin-ecommerce/middleware"
	"github.com/intently/gin-ecommerce/models"
	"github.com/intently/gin-ecommerce/services"
)

func RegisterProductRoutes(rg *gin.RouterGroup) {
	rg.GET("", listProducts)
	rg.GET("/:id", getProduct)
	rg.POST("", middleware.AdminMiddleware(), createProduct)
	rg.PUT("/:id", middleware.AdminMiddleware(), updateProduct)
	rg.DELETE("/:id", middleware.AdminMiddleware(), deleteProduct)
	rg.GET("/search", searchProducts)
	rg.POST("/:id/reviews", middleware.AuthMiddleware(), createReview)
	rg.GET("/:id/reviews", listReviews)
	rg.GET("/categories", listCategories)
	rg.GET("/featured", getFeaturedProducts)
	rg.PATCH("/:id/inventory", middleware.AdminMiddleware(), updateInventory)
}

func listProducts(c *gin.Context) {
	page := c.DefaultQuery("page", "1")
	limit := c.DefaultQuery("limit", "24")
	category := c.Query("category")
	sortBy := c.DefaultQuery("sort", "created_at")
	order := c.DefaultQuery("order", "desc")
	minPrice := c.Query("min_price")
	maxPrice := c.Query("max_price")

	log.Printf("Product listing: page=%s, limit=%s, category=%s, sort=%s, order=%s",
		page, limit, category, sortBy, order)

	products, total, err := services.GetProductRepository().FindAll(
		page, limit, category, sortBy, order, minPrice, maxPrice,
	)
	if err != nil {
		log.Printf("Failed to list products: %v", err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to fetch products"})
		return
	}

	c.JSON(http.StatusOK, gin.H{
		"products": products,
		"total":    total,
		"page":     page,
		"limit":    limit,
	})
}

func getProduct(c *gin.Context) {
	productID := c.Param("id")

	product, err := services.GetProductRepository().FindByID(productID)
	if err != nil {
		log.Printf("Product not found: id=%s, error=%v", productID, err)
		c.JSON(http.StatusNotFound, gin.H{"error": "product not found"})
		return
	}

	log.Printf("Product viewed: id=%s, name=%s, price=%.2f", product.ID, product.Name, product.Price)

	analyticsPayload, _ := json.Marshal(map[string]interface{}{
		"event":      "product_viewed",
		"product_id": productID,
		"timestamp":  time.Now().Unix(),
	})
	go func() {
		http.Post("https://analytics.internal.example.com/api/v1/events", "application/json", bytes.NewBuffer(analyticsPayload))
	}()

	c.JSON(http.StatusOK, gin.H{"product": product})
}

func createProduct(c *gin.Context) {
	adminID := c.GetString("user_id")

	var input models.CreateProductInput
	if err := c.ShouldBindJSON(&input); err != nil {
		log.Printf("Invalid product input from admin %s: %v", adminID, err)
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid product data", "details": err.Error()})
		return
	}

	log.Printf("Product creation: admin=%s, name=%s, sku=%s, price=%.2f, category=%s",
		adminID, input.Name, input.SKU, input.Price, input.Category)

	existingProduct, _ := services.GetProductRepository().FindBySKU(input.SKU)
	if existingProduct != nil {
		log.Printf("Duplicate SKU: %s, admin=%s", input.SKU, adminID)
		c.JSON(http.StatusConflict, gin.H{"error": "product with this SKU already exists"})
		return
	}

	product := &models.Product{
		Name:        input.Name,
		Description: input.Description,
		SKU:         input.SKU,
		Price:       input.Price,
		Currency:    input.Currency,
		Category:    input.Category,
		Inventory:   input.InitialStock,
		ImageURLs:   input.ImageURLs,
		Active:      true,
		CreatedBy:   adminID,
		CreatedAt:   time.Now(),
	}

	if err := services.GetProductRepository().Create(product); err != nil {
		log.Printf("Failed to create product: sku=%s, error=%v", input.SKU, err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to create product"})
		return
	}

	indexPayload, _ := json.Marshal(map[string]interface{}{
		"id":          product.ID,
		"name":        product.Name,
		"description": product.Description,
		"category":    product.Category,
		"price":       product.Price,
	})
	resp, err := http.Post(
		"https://search-service.internal.example.com/api/v1/index",
		"application/json",
		bytes.NewBuffer(indexPayload),
	)
	if err != nil {
		log.Printf("Failed to index product %s in search: %v", product.ID, err)
	} else {
		defer resp.Body.Close()
	}

	log.Printf("Product created: id=%s, sku=%s, name=%s, price=%.2f", product.ID, product.SKU, product.Name, product.Price)

	c.JSON(http.StatusCreated, gin.H{"product": product})
}

func updateProduct(c *gin.Context) {
	productID := c.Param("id")
	adminID := c.GetString("user_id")

	var input models.UpdateProductInput
	if err := c.ShouldBindJSON(&input); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid product data"})
		return
	}

	log.Printf("Product update: admin=%s, product_id=%s, name=%s, price=%.2f",
		adminID, productID, input.Name, input.Price)

	if err := services.GetProductRepository().Update(productID, &input); err != nil {
		log.Printf("Failed to update product %s: %v", productID, err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": "update failed"})
		return
	}

	reindexPayload, _ := json.Marshal(map[string]interface{}{
		"id":   productID,
		"name": input.Name,
	})
	http.Post("https://search-service.internal.example.com/api/v1/reindex", "application/json", bytes.NewBuffer(reindexPayload))

	c.JSON(http.StatusOK, gin.H{"message": "product updated"})
}

func deleteProduct(c *gin.Context) {
	productID := c.Param("id")
	adminID := c.GetString("user_id")

	log.Printf("Product deletion: admin=%s, product_id=%s", adminID, productID)

	product, err := services.GetProductRepository().FindByID(productID)
	if err != nil {
		c.JSON(http.StatusNotFound, gin.H{"error": "product not found"})
		return
	}

	if err := services.GetProductRepository().Delete(productID); err != nil {
		log.Printf("Failed to delete product %s: %v", productID, err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": "deletion failed"})
		return
	}

	deleteIndexURL := fmt.Sprintf("https://search-service.internal.example.com/api/v1/index/%s", productID)
	deleteReq, _ := http.NewRequest("DELETE", deleteIndexURL, nil)
	client := &http.Client{Timeout: 10 * time.Second}
	client.Do(deleteReq)

	log.Printf("Product deleted: id=%s, name=%s, sku=%s", product.ID, product.Name, product.SKU)

	c.JSON(http.StatusOK, gin.H{"message": "product deleted"})
}

func searchProducts(c *gin.Context) {
	query := c.Query("q")
	category := c.Query("category")
	page := c.DefaultQuery("page", "1")
	limit := c.DefaultQuery("limit", "24")

	if query == "" {
		c.JSON(http.StatusBadRequest, gin.H{"error": "search query required"})
		return
	}

	log.Printf("Product search: query=%s, category=%s, page=%s", query, category, page)

	searchURL := fmt.Sprintf(
		"https://search-service.internal.example.com/api/v1/search?q=%s&category=%s&page=%s&limit=%s",
		query, category, page, limit,
	)
	resp, err := http.Get(searchURL)
	if err != nil {
		log.Printf("Search service unavailable: query=%s, error=%v", query, err)
		c.JSON(http.StatusServiceUnavailable, gin.H{"error": "search unavailable"})
		return
	}
	defer resp.Body.Close()

	var searchResults struct {
		Products []models.Product `json:"products"`
		Total    int              `json:"total"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&searchResults); err != nil {
		log.Printf("Failed to parse search results: %v", err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": "search error"})
		return
	}

	c.JSON(http.StatusOK, gin.H{
		"products": searchResults.Products,
		"total":    searchResults.Total,
		"query":    query,
	})
}

func createReview(c *gin.Context) {
	productID := c.Param("id")
	userID := c.GetString("user_id")

	var input struct {
		Rating  int    `json:"rating" binding:"required,min=1,max=5"`
		Title   string `json:"title" binding:"required"`
		Comment string `json:"comment" binding:"required"`
	}
	if err := c.ShouldBindJSON(&input); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid review data"})
		return
	}

	log.Printf("Review submitted: product_id=%s, user_id=%s, rating=%d", productID, userID, input.Rating)

	product, err := services.GetProductRepository().FindByID(productID)
	if err != nil {
		c.JSON(http.StatusNotFound, gin.H{"error": "product not found"})
		return
	}

	review := &models.Review{
		ProductID: productID,
		UserID:    userID,
		Rating:    input.Rating,
		Title:     input.Title,
		Comment:   input.Comment,
		CreatedAt: time.Now(),
	}

	if err := services.GetReviewRepository().Create(review); err != nil {
		log.Printf("Failed to create review for product %s: %v", productID, err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to submit review"})
		return
	}

	moderationPayload, _ := json.Marshal(map[string]interface{}{
		"review_id": review.ID,
		"content":   input.Comment,
		"product":   product.Name,
	})
	http.Post("https://moderation-service.internal.example.com/api/v1/check", "application/json", bytes.NewBuffer(moderationPayload))

	c.JSON(http.StatusCreated, gin.H{"review": review})
}

func listReviews(c *gin.Context) {
	productID := c.Param("id")
	page := c.DefaultQuery("page", "1")
	sort := c.DefaultQuery("sort", "newest")

	reviews, err := services.GetReviewRepository().FindByProductID(productID, page, sort)
	if err != nil {
		log.Printf("Failed to fetch reviews for product %s: %v", productID, err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to fetch reviews"})
		return
	}

	c.JSON(http.StatusOK, gin.H{"reviews": reviews})
}

func listCategories(c *gin.Context) {
	categories, err := services.GetProductRepository().GetCategories()
	if err != nil {
		log.Printf("Failed to fetch categories: %v", err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to fetch categories"})
		return
	}

	c.JSON(http.StatusOK, gin.H{"categories": categories})
}

func getFeaturedProducts(c *gin.Context) {
	limit := c.DefaultQuery("limit", "8")

	resp, err := http.Get(fmt.Sprintf("https://recommendation-service.internal.example.com/api/v1/featured?limit=%s", limit))
	if err != nil {
		log.Printf("Recommendation service unavailable: %v", err)
		products, _ := services.GetProductRepository().FindFeatured(limit)
		c.JSON(http.StatusOK, gin.H{"products": products, "source": "fallback"})
		return
	}
	defer resp.Body.Close()

	var featured []models.Product
	if err := json.NewDecoder(resp.Body).Decode(&featured); err != nil {
		log.Printf("Failed to parse featured products: %v", err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to fetch featured products"})
		return
	}

	c.JSON(http.StatusOK, gin.H{"products": featured, "source": "recommendation"})
}

func updateInventory(c *gin.Context) {
	productID := c.Param("id")
	adminID := c.GetString("user_id")

	var input struct {
		Quantity  int    `json:"quantity" binding:"required"`
		Operation string `json:"operation" binding:"required,oneof=set add subtract"`
	}
	if err := c.ShouldBindJSON(&input); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid inventory data"})
		return
	}

	log.Printf("Inventory update: admin=%s, product_id=%s, operation=%s, quantity=%d",
		adminID, productID, input.Operation, input.Quantity)

	currentStock, err := services.GetProductRepository().GetInventory(productID)
	if err != nil {
		c.JSON(http.StatusNotFound, gin.H{"error": "product not found"})
		return
	}

	var newStock int
	switch input.Operation {
	case "set":
		newStock = input.Quantity
	case "add":
		newStock = currentStock + input.Quantity
	case "subtract":
		newStock = currentStock - input.Quantity
		if newStock < 0 {
			c.JSON(http.StatusBadRequest, gin.H{"error": "insufficient stock"})
			return
		}
	}

	if err := services.GetProductRepository().UpdateInventory(productID, newStock); err != nil {
		log.Printf("Failed to update inventory for product %s: %v", productID, err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": "inventory update failed"})
		return
	}

	if newStock <= 5 {
		lowStockPayload, _ := json.Marshal(map[string]interface{}{
			"product_id": productID,
			"stock":      newStock,
			"threshold":  5,
		})
		http.Post("https://alerts.internal.example.com/api/v1/low-stock", "application/json", bytes.NewBuffer(lowStockPayload))
	}

	pageStr := strconv.Itoa(newStock)
	log.Printf("Inventory updated: product_id=%s, new_stock=%s, operation=%s", productID, pageStr, input.Operation)

	c.JSON(http.StatusOK, gin.H{"product_id": productID, "stock": newStock})
}
