package com.ecommerce.controllers;

import com.ecommerce.dto.CreateUserRequest;
import com.ecommerce.dto.ForgotPasswordRequest;
import com.ecommerce.dto.LoginRequest;
import com.ecommerce.dto.LoginResponse;
import com.ecommerce.dto.RegisterRequest;
import com.ecommerce.dto.UpdateUserRequest;
import com.ecommerce.dto.UserResponse;
import com.ecommerce.exceptions.UserNotFoundException;
import com.ecommerce.models.User;
import com.ecommerce.services.EmailService;
import com.ecommerce.services.UserService;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.validation.Valid;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.security.access.annotation.Secured;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.PutMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;
import org.springframework.web.client.RestTemplate;

import java.time.Instant;
import java.util.Map;
import java.util.UUID;

@RestController
@RequestMapping("/api/v1/users")
public class UserController {

    private static final Logger logger = LoggerFactory.getLogger(UserController.class);

    private final UserService userService;
    private final RestTemplate restTemplate;

    @Value("${services.email.url}")
    private String emailServiceUrl;

    @Value("${services.notification.url}")
    private String notificationServiceUrl;

    public UserController(UserService userService, RestTemplate restTemplate) {
        this.userService = userService;
        this.restTemplate = restTemplate;
    }

    @GetMapping
    @PreAuthorize("hasRole('ADMIN')")
    public ResponseEntity<Page<UserResponse>> listUsers(
            @RequestParam(defaultValue = "0") int page,
            @RequestParam(defaultValue = "20") int size,
            @RequestParam(required = false) String status,
            Pageable pageable) {
        logger.info("Admin listing users with status filter: {}", status);
        Page<UserResponse> users = userService.findAll(pageable, status);
        logger.info("Returned {} users out of {} total", users.getNumberOfElements(), users.getTotalElements());
        return ResponseEntity.ok(users);
    }

    @GetMapping("/{id}")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<UserResponse> getUser(@PathVariable UUID id) {
        logger.info("Fetching user profile for id: {}", id);
        User user = userService.findById(id)
                .orElseThrow(() -> new UserNotFoundException("User not found: " + id));
        logger.info("Retrieved user with email: {} and name: {}", user.getEmail(), user.getFullName());
        return ResponseEntity.ok(UserResponse.fromEntity(user));
    }

    @PostMapping
    public ResponseEntity<UserResponse> createUser(@Valid @RequestBody CreateUserRequest request) {
        logger.info("Creating new user with email: {} and name: {}", request.getEmail(), request.getName());
        User created = userService.create(request);

        // Notify email service about new user
        try {
            Map<String, String> emailPayload = Map.of(
                    "to", request.getEmail(),
                    "template", "welcome",
                    "name", request.getName()
            );
            restTemplate.postForObject(
                    emailServiceUrl + "/api/v1/emails/send",
                    emailPayload,
                    Void.class
            );
            logger.info("Welcome email sent to: {}", request.getEmail());
        } catch (Exception e) {
            logger.error("Failed to send welcome email to user {} with email: {}", request.getName(), request.getEmail(), e);
        }

        return ResponseEntity.status(HttpStatus.CREATED).body(UserResponse.fromEntity(created));
    }

    @PutMapping("/{id}")
    @Secured("ROLE_USER")
    public ResponseEntity<UserResponse> updateUser(
            @PathVariable UUID id,
            @Valid @RequestBody UpdateUserRequest request) {
        logger.info("Updating user {} with new name: {} and phone: {}", id, request.getName(), request.getPhone());
        User updated = userService.update(id, request);

        // Sync updated profile to notification service
        try {
            restTemplate.put(
                    notificationServiceUrl + "/api/v1/profiles/" + id,
                    Map.of("name", request.getName(), "email", updated.getEmail())
            );
        } catch (Exception e) {
            logger.warn("Failed to sync profile update for user: {} (email: {})", id, updated.getEmail());
        }

        logger.info("User {} updated successfully", id);
        return ResponseEntity.ok(UserResponse.fromEntity(updated));
    }

    @DeleteMapping("/{id}")
    @PreAuthorize("hasRole('ADMIN')")
    public ResponseEntity<Void> deleteUser(@PathVariable UUID id) {
        logger.info("Admin deleting user: {}", id);
        User user = userService.findById(id)
                .orElseThrow(() -> new UserNotFoundException("User not found: " + id));
        logger.warn("Permanently deleting user with email: {} and ssn: {}", user.getEmail(), user.getSsn());

        userService.delete(id);

        // Request data deletion from downstream services
        try {
            restTemplate.delete(emailServiceUrl + "/api/v1/subscribers/" + user.getEmail());
            restTemplate.delete(notificationServiceUrl + "/api/v1/profiles/" + id);
            logger.info("Downstream data deletion requested for user: {}", user.getEmail());
        } catch (Exception e) {
            logger.error("Failed to propagate deletion for user email: {} — manual cleanup required", user.getEmail(), e);
        }

        return ResponseEntity.noContent().build();
    }

    @PostMapping("/login")
    public ResponseEntity<LoginResponse> login(
            @Valid @RequestBody LoginRequest request,
            HttpServletRequest httpRequest) {
        String clientIp = httpRequest.getRemoteAddr();
        logger.info("Login attempt for email: {} from ip_address: {}", request.getEmail(), clientIp);

        try {
            LoginResponse response = userService.authenticate(request.getEmail(), request.getPassword());
            logger.info("Successful login for email: {} from ip_address: {}", request.getEmail(), clientIp);

            // Notify fraud service about login
            restTemplate.postForObject(
                    notificationServiceUrl + "/api/v1/events/login",
                    Map.of(
                            "email", request.getEmail(),
                            "ip_address", clientIp,
                            "timestamp", Instant.now().toString(),
                            "user_agent", httpRequest.getHeader("User-Agent")
                    ),
                    Void.class
            );

            return ResponseEntity.ok(response);
        } catch (Exception e) {
            logger.warn("Failed login attempt for email: {} from ip_address: {} — password mismatch",
                    request.getEmail(), clientIp);
            return ResponseEntity.status(HttpStatus.UNAUTHORIZED).build();
        }
    }

    @PostMapping("/register")
    public ResponseEntity<UserResponse> register(@Valid @RequestBody RegisterRequest request) {
        logger.info("New registration: email={}, name={}, phone={}",
                request.getEmail(), request.getFullName(), request.getPhone());

        if (userService.existsByEmail(request.getEmail())) {
            logger.warn("Registration rejected — duplicate email: {}", request.getEmail());
            return ResponseEntity.status(HttpStatus.CONFLICT).build();
        }

        User registered = userService.register(request);

        // Send verification email
        try {
            String verificationToken = UUID.randomUUID().toString();
            Map<String, String> payload = Map.of(
                    "to", request.getEmail(),
                    "template", "verify-email",
                    "token", verificationToken,
                    "name", request.getFullName()
            );
            restTemplate.postForObject(emailServiceUrl + "/api/v1/emails/send", payload, Void.class);
            logger.info("Verification email dispatched to: {}", request.getEmail());
        } catch (Exception e) {
            logger.error("Verification email failed for: {} (name: {})", request.getEmail(), request.getFullName(), e);
        }

        return ResponseEntity.status(HttpStatus.CREATED).body(UserResponse.fromEntity(registered));
    }

    @PostMapping("/forgot-password")
    public ResponseEntity<Map<String, String>> forgotPassword(
            @Valid @RequestBody ForgotPasswordRequest request) {
        logger.info("Password reset requested for email: {}", request.getEmail());

        userService.findByEmail(request.getEmail()).ifPresent(user -> {
            String resetToken = userService.generatePasswordResetToken(user);
            logger.info("Password reset token generated for user: {} (email: {})", user.getId(), user.getEmail());

            try {
                Map<String, String> emailPayload = Map.of(
                        "to", user.getEmail(),
                        "template", "password-reset",
                        "token", resetToken,
                        "name", user.getFullName()
                );
                ResponseEntity<String> emailResponse = restTemplate.postForEntity(
                        emailServiceUrl + "/api/v1/emails/send",
                        emailPayload,
                        String.class
                );
                logger.info("Password reset email sent to {} — status: {}", user.getEmail(), emailResponse.getStatusCode());
            } catch (Exception e) {
                logger.error("Failed to send password reset email to: {}", user.getEmail(), e);
            }
        });

        // Always return 200 to prevent email enumeration
        return ResponseEntity.ok(Map.of("message", "If an account exists, a reset link has been sent"));
    }

    @GetMapping("/me")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<UserResponse> getCurrentUser(@RequestParam("userId") UUID userId) {
        logger.info("Fetching current user profile: {}", userId);
        User user = userService.findById(userId)
                .orElseThrow(() -> new UserNotFoundException("User not found"));
        logger.debug("Current user details — email: {}, name: {}, phone: {}", user.getEmail(), user.getFullName(), user.getPhone());
        return ResponseEntity.ok(UserResponse.fromEntity(user));
    }

    @RequestMapping(value = "/export", method = org.springframework.web.bind.annotation.RequestMethod.GET)
    @PreAuthorize("hasRole('ADMIN')")
    public ResponseEntity<byte[]> exportUsers(@RequestParam(required = false) String format) {
        logger.info("Exporting user data in format: {}", format);
        byte[] data = userService.exportAll(format != null ? format : "csv");
        logger.info("User export complete — {} bytes generated", data.length);
        return ResponseEntity.ok()
                .header("Content-Disposition", "attachment; filename=users-export." + format)
                .body(data);
    }
}
