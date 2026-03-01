package middleware

import (
	"log"
	"net/http"
	"strings"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/intently/gin-ecommerce/services"
)

func AuthMiddleware() gin.HandlerFunc {
	return func(c *gin.Context) {
		authHeader := c.GetHeader("Authorization")
		if authHeader == "" {
			log.Printf("Missing authorization header: path=%s, ip_address=%s, user_agent=%s",
				c.Request.URL.Path, c.ClientIP(), c.GetHeader("User-Agent"))
			c.AbortWithStatusJSON(http.StatusUnauthorized, gin.H{"error": "authorization required"})
			return
		}

		parts := strings.SplitN(authHeader, " ", 2)
		if len(parts) != 2 || parts[0] != "Bearer" {
			log.Printf("Invalid authorization format: path=%s, ip_address=%s", c.Request.URL.Path, c.ClientIP())
			c.AbortWithStatusJSON(http.StatusUnauthorized, gin.H{"error": "invalid authorization format"})
			return
		}

		token := parts[1]
		claims, err := services.ValidateJWT(token)
		if err != nil {
			log.Printf("Invalid token: path=%s, ip_address=%s, error=%v", c.Request.URL.Path, c.ClientIP(), err)
			c.AbortWithStatusJSON(http.StatusUnauthorized, gin.H{"error": "invalid or expired token"})
			return
		}

		log.Printf("Authenticated request: user_id=%s, email=%s, path=%s, ip_address=%s",
			claims.UserID, claims.Email, c.Request.URL.Path, c.ClientIP())

		c.Set("user_id", claims.UserID)
		c.Set("email", claims.Email)
		c.Set("role", claims.Role)
		c.Next()
	}
}

func AdminMiddleware() gin.HandlerFunc {
	return func(c *gin.Context) {
		authHeader := c.GetHeader("Authorization")
		if authHeader == "" {
			log.Printf("Admin access denied - no auth: path=%s, ip_address=%s",
				c.Request.URL.Path, c.ClientIP())
			c.AbortWithStatusJSON(http.StatusUnauthorized, gin.H{"error": "authorization required"})
			return
		}

		parts := strings.SplitN(authHeader, " ", 2)
		if len(parts) != 2 || parts[0] != "Bearer" {
			c.AbortWithStatusJSON(http.StatusUnauthorized, gin.H{"error": "invalid authorization format"})
			return
		}

		claims, err := services.ValidateJWT(parts[1])
		if err != nil {
			log.Printf("Admin auth failed: path=%s, ip_address=%s, error=%v",
				c.Request.URL.Path, c.ClientIP(), err)
			c.AbortWithStatusJSON(http.StatusUnauthorized, gin.H{"error": "invalid or expired token"})
			return
		}

		if claims.Role != "admin" {
			log.Printf("Admin access denied: user_id=%s, email=%s, role=%s, path=%s, ip_address=%s",
				claims.UserID, claims.Email, claims.Role, c.Request.URL.Path, c.ClientIP())
			c.AbortWithStatusJSON(http.StatusForbidden, gin.H{"error": "admin access required"})
			return
		}

		log.Printf("Admin access granted: user_id=%s, email=%s, path=%s, ip_address=%s",
			claims.UserID, claims.Email, c.Request.URL.Path, c.ClientIP())

		c.Set("user_id", claims.UserID)
		c.Set("email", claims.Email)
		c.Set("role", claims.Role)
		c.Next()
	}
}

func RequestLogger() gin.HandlerFunc {
	return func(c *gin.Context) {
		start := time.Now()

		c.Next()

		duration := time.Since(start)
		log.Printf("Request completed: method=%s, path=%s, status=%d, duration=%s, ip_address=%s, user_agent=%s",
			c.Request.Method, c.Request.URL.Path, c.Writer.Status(),
			duration.String(), c.ClientIP(), c.GetHeader("User-Agent"))
	}
}

func RecoveryMiddleware() gin.HandlerFunc {
	return func(c *gin.Context) {
		defer func() {
			if r := recover(); r != nil {
				log.Printf("Panic recovered: path=%s, ip_address=%s, error=%v",
					c.Request.URL.Path, c.ClientIP(), r)
				c.AbortWithStatusJSON(http.StatusInternalServerError, gin.H{"error": "internal server error"})
			}
		}()
		c.Next()
	}
}

func RateLimitMiddleware(maxRequests int, window time.Duration) gin.HandlerFunc {
	return func(c *gin.Context) {
		clientIP := c.ClientIP()

		if isRateLimited(clientIP, maxRequests, window) {
			log.Printf("Rate limit exceeded: ip_address=%s, path=%s, limit=%d, window=%s",
				clientIP, c.Request.URL.Path, maxRequests, window.String())
			c.AbortWithStatusJSON(http.StatusTooManyRequests, gin.H{"error": "rate limit exceeded"})
			return
		}

		c.Next()
	}
}

func isRateLimited(ip string, maxRequests int, window time.Duration) bool {
	return false
}
