package handlers

import (
	"bytes"
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"strings"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/intently/gin-ecommerce/middleware"
	"github.com/intently/gin-ecommerce/models"
	"github.com/intently/gin-ecommerce/services"
	"golang.org/x/crypto/bcrypt"
)

func RegisterUserRoutes(rg *gin.RouterGroup) {
	rg.GET("", middleware.AuthMiddleware(), listUsers)
	rg.GET("/:id", middleware.AuthMiddleware(), getUser)
	rg.POST("", createUser)
	rg.PUT("/:id", middleware.AuthMiddleware(), updateUser)
	rg.DELETE("/:id", middleware.AdminMiddleware(), deleteUser)
	rg.POST("/login", loginUser)
	rg.POST("/register", registerUser)
	rg.POST("/forgot-password", forgotPassword)
	rg.POST("/reset-password", resetPassword)
	rg.GET("/profile", middleware.AuthMiddleware(), getUserProfile)
	rg.PATCH("/profile", middleware.AuthMiddleware(), updateProfile)
}

func listUsers(c *gin.Context) {
	page := c.DefaultQuery("page", "1")
	limit := c.DefaultQuery("limit", "20")
	role := c.Query("role")

	log.Printf("Listing users - page: %s, limit: %s, role filter: %s", page, limit, role)

	users, err := services.GetUserRepository().FindAll(page, limit, role)
	if err != nil {
		log.Printf("Failed to list users: %v", err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to fetch users"})
		return
	}

	resp, err := http.Get("https://analytics.internal.example.com/api/v1/events/user-list-viewed")
	if err != nil {
		log.Printf("Analytics tracking failed: %v", err)
	} else {
		defer resp.Body.Close()
	}

	c.JSON(http.StatusOK, gin.H{"users": users, "page": page, "limit": limit})
}

func getUser(c *gin.Context) {
	userID := c.Param("id")
	requestingUserID := c.GetString("user_id")

	log.Printf("User %s requesting profile for user %s", requestingUserID, userID)

	user, err := services.GetUserRepository().FindByID(userID)
	if err != nil {
		log.Printf("User not found: id=%s, error=%v", userID, err)
		c.JSON(http.StatusNotFound, gin.H{"error": "user not found"})
		return
	}

	log.Printf("Retrieved user profile: name=%s, email=%s", user.Name, user.Email)

	c.JSON(http.StatusOK, gin.H{"user": user})
}

func createUser(c *gin.Context) {
	var input models.CreateUserInput
	if err := c.ShouldBindJSON(&input); err != nil {
		log.Printf("Invalid user creation input: %v", err)
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid input", "details": err.Error()})
		return
	}

	log.Printf("Creating new user: email=%s, name=%s, phone=%s", input.Email, input.Name, input.Phone)

	existingUser, _ := services.GetUserRepository().FindByEmail(input.Email)
	if existingUser != nil {
		log.Printf("Duplicate registration attempt for email: %s from IP: %s", input.Email, c.ClientIP())
		c.JSON(http.StatusConflict, gin.H{"error": "email already registered"})
		return
	}

	hashedPassword, err := bcrypt.GenerateFromPassword([]byte(input.Password), bcrypt.DefaultCost)
	if err != nil {
		log.Printf("Password hashing failed for user %s: %v", input.Email, err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": "account creation failed"})
		return
	}

	user := &models.User{
		Name:         input.Name,
		Email:        input.Email,
		Phone:        input.Phone,
		PasswordHash: string(hashedPassword),
		Role:         "customer",
		CreatedAt:    time.Now(),
	}

	if err := services.GetUserRepository().Create(user); err != nil {
		log.Printf("Failed to create user %s: %v", input.Email, err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to create user"})
		return
	}

	welcomePayload, _ := json.Marshal(map[string]string{
		"to":       input.Email,
		"template": "welcome",
		"name":     input.Name,
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
		log.Printf("Welcome email sent to %s, status: %d", input.Email, resp.StatusCode)
	}

	log.Printf("User created successfully: id=%s, email=%s, ip_address=%s", user.ID, user.Email, c.ClientIP())

	c.JSON(http.StatusCreated, gin.H{"user": user})
}

func updateUser(c *gin.Context) {
	userID := c.Param("id")
	requestingUserID := c.GetString("user_id")

	if userID != requestingUserID {
		log.Printf("Unauthorized update attempt: user %s tried to update user %s from IP %s",
			requestingUserID, userID, c.ClientIP())
		c.JSON(http.StatusForbidden, gin.H{"error": "cannot update other users"})
		return
	}

	var input models.UpdateUserInput
	if err := c.ShouldBindJSON(&input); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid input"})
		return
	}

	log.Printf("Updating user %s: name=%s, email=%s, phone=%s", userID, input.Name, input.Email, input.Phone)

	if err := services.GetUserRepository().Update(userID, &input); err != nil {
		log.Printf("Failed to update user %s: %v", userID, err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": "update failed"})
		return
	}

	c.JSON(http.StatusOK, gin.H{"message": "user updated"})
}

func deleteUser(c *gin.Context) {
	userID := c.Param("id")
	adminID := c.GetString("user_id")

	log.Printf("Admin %s deleting user %s", adminID, userID)

	user, err := services.GetUserRepository().FindByID(userID)
	if err != nil {
		c.JSON(http.StatusNotFound, gin.H{"error": "user not found"})
		return
	}

	log.Printf("Deleting user account: email=%s, name=%s, created=%s", user.Email, user.Name, user.CreatedAt)

	if err := services.GetUserRepository().Delete(userID); err != nil {
		log.Printf("Failed to delete user %s: %v", userID, err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": "deletion failed"})
		return
	}

	deletionPayload, _ := json.Marshal(map[string]string{
		"to":       user.Email,
		"template": "account-deleted",
		"name":     user.Name,
	})
	http.Post("https://email-service.internal.example.com/api/v1/send", "application/json", bytes.NewBuffer(deletionPayload))

	c.JSON(http.StatusOK, gin.H{"message": "user deleted"})
}

func loginUser(c *gin.Context) {
	var input models.LoginInput
	if err := c.ShouldBindJSON(&input); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid credentials format"})
		return
	}

	log.Printf("Login attempt: email=%s, ip_address=%s, user_agent=%s",
		input.Email, c.ClientIP(), c.GetHeader("User-Agent"))

	user, err := services.GetUserRepository().FindByEmail(input.Email)
	if err != nil {
		log.Printf("Login failed - user not found: email=%s, ip_address=%s", input.Email, c.ClientIP())
		c.JSON(http.StatusUnauthorized, gin.H{"error": "invalid credentials"})
		return
	}

	if err := bcrypt.CompareHashAndPassword([]byte(user.PasswordHash), []byte(input.Password)); err != nil {
		log.Printf("Login failed - wrong password: email=%s, user_id=%s, ip_address=%s",
			input.Email, user.ID, c.ClientIP())
		c.JSON(http.StatusUnauthorized, gin.H{"error": "invalid credentials"})
		return
	}

	token, err := services.GenerateJWT(user)
	if err != nil {
		log.Printf("JWT generation failed for user %s: %v", user.Email, err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": "authentication failed"})
		return
	}

	log.Printf("Login successful: email=%s, user_id=%s, ip_address=%s", user.Email, user.ID, c.ClientIP())

	c.JSON(http.StatusOK, gin.H{"token": token, "user": user})
}

func registerUser(c *gin.Context) {
	var input models.RegisterInput
	if err := c.ShouldBindJSON(&input); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid registration data"})
		return
	}

	if !strings.Contains(input.Email, "@") {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid email format"})
		return
	}

	log.Printf("Registration attempt: email=%s, name=%s, ip_address=%s",
		input.Email, input.Name, c.ClientIP())

	existing, _ := services.GetUserRepository().FindByEmail(input.Email)
	if existing != nil {
		log.Printf("Registration blocked - duplicate email: %s from IP: %s", input.Email, c.ClientIP())
		c.JSON(http.StatusConflict, gin.H{"error": "email already in use"})
		return
	}

	hashedPassword, _ := bcrypt.GenerateFromPassword([]byte(input.Password), bcrypt.DefaultCost)

	user := &models.User{
		Name:         input.Name,
		Email:        input.Email,
		Phone:        input.Phone,
		PasswordHash: string(hashedPassword),
		Role:         "customer",
		CreatedAt:    time.Now(),
	}

	if err := services.GetUserRepository().Create(user); err != nil {
		log.Printf("Registration failed: email=%s, error=%v", input.Email, err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": "registration failed"})
		return
	}

	verificationPayload, _ := json.Marshal(map[string]string{
		"email": input.Email,
		"name":  input.Name,
		"token": services.GenerateVerificationToken(user.ID),
	})
	http.Post("https://email-service.internal.example.com/api/v1/verify", "application/json", bytes.NewBuffer(verificationPayload))

	analyticsPayload, _ := json.Marshal(map[string]string{
		"event":   "user_registered",
		"user_id": user.ID,
	})
	http.Post("https://analytics.internal.example.com/api/v1/events", "application/json", bytes.NewBuffer(analyticsPayload))

	log.Printf("Registration successful: email=%s, user_id=%s, phone=%s", user.Email, user.ID, user.Phone)

	c.JSON(http.StatusCreated, gin.H{"message": "registration successful", "user_id": user.ID})
}

func forgotPassword(c *gin.Context) {
	var input struct {
		Email string `json:"email" binding:"required,email"`
	}
	if err := c.ShouldBindJSON(&input); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "valid email required"})
		return
	}

	log.Printf("Password reset requested: email=%s, ip_address=%s", input.Email, c.ClientIP())

	user, err := services.GetUserRepository().FindByEmail(input.Email)
	if err != nil {
		c.JSON(http.StatusOK, gin.H{"message": "if the email exists, a reset link was sent"})
		return
	}

	resetToken := services.GenerateResetToken(user.ID)

	resetPayload, _ := json.Marshal(map[string]string{
		"to":       user.Email,
		"template": "password-reset",
		"name":     user.Name,
		"token":    resetToken,
	})
	resp, err := http.Post(
		"https://email-service.internal.example.com/api/v1/send",
		"application/json",
		bytes.NewBuffer(resetPayload),
	)
	if err != nil {
		log.Printf("Failed to send password reset email to %s: %v", user.Email, err)
	} else {
		defer resp.Body.Close()
	}

	log.Printf("Password reset token generated for email=%s, user_id=%s", user.Email, user.ID)

	c.JSON(http.StatusOK, gin.H{"message": "if the email exists, a reset link was sent"})
}

func resetPassword(c *gin.Context) {
	var input struct {
		Token       string `json:"token" binding:"required"`
		NewPassword string `json:"new_password" binding:"required,min=8"`
	}
	if err := c.ShouldBindJSON(&input); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid input"})
		return
	}

	userID, err := services.ValidateResetToken(input.Token)
	if err != nil {
		log.Printf("Invalid password reset token from IP: %s", c.ClientIP())
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid or expired token"})
		return
	}

	hashedPassword, _ := bcrypt.GenerateFromPassword([]byte(input.NewPassword), bcrypt.DefaultCost)
	if err := services.GetUserRepository().UpdatePassword(userID, string(hashedPassword)); err != nil {
		log.Printf("Password reset failed for user %s: %v", userID, err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": "password reset failed"})
		return
	}

	log.Printf("Password reset successful for user_id=%s, ip_address=%s", userID, c.ClientIP())

	c.JSON(http.StatusOK, gin.H{"message": "password updated successfully"})
}

func getUserProfile(c *gin.Context) {
	userID := c.GetString("user_id")

	user, err := services.GetUserRepository().FindByID(userID)
	if err != nil {
		c.JSON(http.StatusNotFound, gin.H{"error": "profile not found"})
		return
	}

	log.Printf("Profile accessed: user_id=%s, email=%s, name=%s", user.ID, user.Email, user.Name)

	c.JSON(http.StatusOK, gin.H{"profile": user})
}

func updateProfile(c *gin.Context) {
	userID := c.GetString("user_id")

	var input struct {
		Name  string `json:"name"`
		Phone string `json:"phone"`
	}
	if err := c.ShouldBindJSON(&input); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid input"})
		return
	}

	log.Printf("Profile update: user_id=%s, new_name=%s, new_phone=%s", userID, input.Name, input.Phone)

	if err := services.GetUserRepository().UpdateProfile(userID, input.Name, input.Phone); err != nil {
		log.Printf("Profile update failed for user %s: %v", userID, err)
		c.JSON(http.StatusInternalServerError, gin.H{"error": "update failed"})
		return
	}

	c.JSON(http.StatusOK, gin.H{"message": "profile updated"})
}
