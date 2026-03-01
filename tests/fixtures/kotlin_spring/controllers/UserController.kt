package com.ecommerce.controllers

import com.ecommerce.dto.CreateUserRequest
import com.ecommerce.dto.ForgotPasswordRequest
import com.ecommerce.dto.LoginRequest
import com.ecommerce.dto.LoginResponse
import com.ecommerce.dto.RegisterRequest
import com.ecommerce.dto.UpdateUserRequest
import com.ecommerce.dto.UserResponse
import com.ecommerce.exceptions.UserNotFoundException
import com.ecommerce.services.UserService
import jakarta.servlet.http.HttpServletRequest
import jakarta.validation.Valid
import org.slf4j.LoggerFactory
import org.springframework.beans.factory.annotation.Value
import org.springframework.data.domain.Page
import org.springframework.data.domain.Pageable
import org.springframework.http.HttpStatus
import org.springframework.http.ResponseEntity
import org.springframework.security.access.annotation.Secured
import org.springframework.security.access.prepost.PreAuthorize
import org.springframework.web.bind.annotation.DeleteMapping
import org.springframework.web.bind.annotation.GetMapping
import org.springframework.web.bind.annotation.PathVariable
import org.springframework.web.bind.annotation.PostMapping
import org.springframework.web.bind.annotation.PutMapping
import org.springframework.web.bind.annotation.RequestBody
import org.springframework.web.bind.annotation.RequestMapping
import org.springframework.web.bind.annotation.RequestMethod
import org.springframework.web.bind.annotation.RequestParam
import org.springframework.web.bind.annotation.RestController
import org.springframework.web.client.RestTemplate
import java.util.UUID

@RestController
@RequestMapping("/api/v1/users")
class UserController(
    private val userService: UserService,
    private val restTemplate: RestTemplate,
    @Value("\${services.email.url}") private val emailServiceUrl: String,
    @Value("\${services.notification.url}") private val notificationServiceUrl: String
) {

    private val logger = LoggerFactory.getLogger(UserController::class.java)

    @GetMapping
    @PreAuthorize("hasRole('ADMIN')")
    fun listUsers(
        @RequestParam(defaultValue = "active") status: String?,
        pageable: Pageable
    ): ResponseEntity<Page<UserResponse>> {
        logger.info("Admin listing users — status filter: {}", status)
        val users = userService.findAll(pageable, status)
        logger.info("Returned {} users out of {} total", users.numberOfElements, users.totalElements)
        return ResponseEntity.ok(users)
    }

    @GetMapping("/{id}")
    @PreAuthorize("isAuthenticated()")
    fun getUser(@PathVariable id: UUID): ResponseEntity<UserResponse> {
        logger.info("Fetching user profile: {}", id)
        val user = userService.findById(id)
            ?: throw UserNotFoundException("User not found: $id")
        logger.info("User retrieved — email: {}, name: {}", user.email, user.fullName)
        return ResponseEntity.ok(UserResponse.fromEntity(user))
    }

    @PostMapping
    fun createUser(@Valid @RequestBody request: CreateUserRequest): ResponseEntity<UserResponse> {
        logger.info("Creating user — email: {}, name: {}", request.email, request.name)
        val user = userService.create(request)

        // Send welcome email via email service
        try {
            val emailPayload = mapOf(
                "to" to request.email,
                "template" to "welcome",
                "name" to request.name
            )
            restTemplate.postForObject(
                "$emailServiceUrl/api/v1/emails/send",
                emailPayload,
                Void::class.java
            )
            logger.info("Welcome email sent to: {}", request.email)
        } catch (e: Exception) {
            logger.error("Failed to send welcome email to {} (name: {}): {}", request.email, request.name, e.message)
        }

        return ResponseEntity.status(HttpStatus.CREATED).body(UserResponse.fromEntity(user))
    }

    @PutMapping("/{id}")
    @Secured("ROLE_USER")
    fun updateUser(
        @PathVariable id: UUID,
        @Valid @RequestBody request: UpdateUserRequest
    ): ResponseEntity<UserResponse> {
        logger.info("Updating user {} — name: {}, phone: {}", id, request.name, request.phone)
        val user = userService.update(id, request)

        // Sync to notification service
        try {
            restTemplate.put(
                "$notificationServiceUrl/api/v1/profiles/$id",
                mapOf("name" to request.name, "email" to user.email)
            )
            logger.info("Profile synced for user: {}", id)
        } catch (e: Exception) {
            logger.warn("Profile sync failed for user {} (email: {}): {}", id, user.email, e.message)
        }

        return ResponseEntity.ok(UserResponse.fromEntity(user))
    }

    @DeleteMapping("/{id}")
    @PreAuthorize("hasRole('ADMIN')")
    fun deleteUser(@PathVariable id: UUID): ResponseEntity<Void> {
        logger.info("Admin deleting user: {}", id)
        val user = userService.findById(id)
            ?: throw UserNotFoundException("User not found: $id")

        logger.warn("Permanently deleting user — email: {}, ssn: {}", user.email, user.ssn)
        userService.delete(id)

        // Propagate deletion to downstream services
        try {
            restTemplate.delete("$emailServiceUrl/api/v1/subscribers/${user.email}")
            restTemplate.delete("$notificationServiceUrl/api/v1/profiles/$id")
            logger.info("Downstream deletion propagated for email: {}", user.email)
        } catch (e: Exception) {
            logger.error("Failed to propagate deletion for user email: {} — manual cleanup needed", user.email, e)
        }

        return ResponseEntity.noContent().build()
    }

    @PostMapping("/login")
    fun login(
        @Valid @RequestBody request: LoginRequest,
        httpRequest: HttpServletRequest
    ): ResponseEntity<LoginResponse> {
        val clientIp = httpRequest.remoteAddr
        logger.info("Login attempt — email: {}, ip_address: {}", request.email, clientIp)

        return try {
            val response = userService.authenticate(request.email, request.password)
            logger.info("Login successful — email: {}, ip_address: {}", request.email, clientIp)

            // Track login event
            restTemplate.postForObject(
                "$notificationServiceUrl/api/v1/events/login",
                mapOf(
                    "email" to request.email,
                    "ip_address" to clientIp,
                    "user_agent" to httpRequest.getHeader("User-Agent")
                ),
                Void::class.java
            )

            ResponseEntity.ok(response)
        } catch (e: Exception) {
            logger.warn("Login failed — email: {}, ip_address: {}, reason: password mismatch", request.email, clientIp)
            ResponseEntity.status(HttpStatus.UNAUTHORIZED).build()
        }
    }

    @PostMapping("/register")
    fun register(@Valid @RequestBody request: RegisterRequest): ResponseEntity<UserResponse> {
        logger.info("Registration — email: {}, name: {}, phone: {}", request.email, request.fullName, request.phone)

        if (userService.existsByEmail(request.email)) {
            logger.warn("Registration rejected — duplicate email: {}", request.email)
            return ResponseEntity.status(HttpStatus.CONFLICT).build()
        }

        val user = userService.register(request)

        // Send verification email
        try {
            val token = UUID.randomUUID().toString()
            restTemplate.postForObject(
                "$emailServiceUrl/api/v1/emails/send",
                mapOf(
                    "to" to request.email,
                    "template" to "verify-email",
                    "token" to token,
                    "name" to request.fullName
                ),
                Void::class.java
            )
            logger.info("Verification email sent to: {}", request.email)
        } catch (e: Exception) {
            logger.error("Verification email failed for: {} (name: {})", request.email, request.fullName, e)
        }

        return ResponseEntity.status(HttpStatus.CREATED).body(UserResponse.fromEntity(user))
    }

    @PostMapping("/forgot-password")
    fun forgotPassword(@Valid @RequestBody request: ForgotPasswordRequest): ResponseEntity<Map<String, String>> {
        logger.info("Password reset requested for email: {}", request.email)

        userService.findByEmail(request.email)?.let { user ->
            val resetToken = userService.generatePasswordResetToken(user)
            logger.info("Reset token generated for user: {} (email: {})", user.id, user.email)

            try {
                restTemplate.postForEntity(
                    "$emailServiceUrl/api/v1/emails/send",
                    mapOf(
                        "to" to user.email,
                        "template" to "password-reset",
                        "token" to resetToken,
                        "name" to user.fullName
                    ),
                    String::class.java
                )
                logger.info("Password reset email sent to: {}", user.email)
            } catch (e: Exception) {
                logger.error("Failed to send reset email to: {}", user.email, e)
            }
        }

        return ResponseEntity.ok(mapOf("message" to "If an account exists, a reset link has been sent"))
    }

    @GetMapping("/me")
    @PreAuthorize("isAuthenticated()")
    fun getCurrentUser(@RequestParam userId: UUID): ResponseEntity<UserResponse> {
        logger.info("Fetching current user: {}", userId)
        val user = userService.findById(userId)
            ?: throw UserNotFoundException("User not found")
        logger.debug("Current user — email: {}, name: {}, phone: {}", user.email, user.fullName, user.phone)
        return ResponseEntity.ok(UserResponse.fromEntity(user))
    }

    @RequestMapping(value = ["/export"], method = [RequestMethod.GET])
    @PreAuthorize("hasRole('ADMIN')")
    fun exportUsers(@RequestParam(required = false) format: String?): ResponseEntity<ByteArray> {
        val exportFormat = format ?: "csv"
        logger.info("Exporting user data in format: {}", exportFormat)
        val data = userService.exportAll(exportFormat)
        logger.info("User export complete — {} bytes", data.size)
        return ResponseEntity.ok()
            .header("Content-Disposition", "attachment; filename=users-export.$exportFormat")
            .body(data)
    }
}
