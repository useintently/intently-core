using System;
using System.Collections.Generic;
using System.Net.Http;
using System.Net.Http.Json;
using System.Security.Claims;
using System.Threading.Tasks;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Logging;
using EcommerceApi.Models;
using EcommerceApi.Services;
using EcommerceApi.DTOs;

namespace EcommerceApi.Controllers
{
    [ApiController]
    [Route("api/v1/[controller]")]
    public class UsersController : ControllerBase
    {
        private readonly IUserRepository _userRepository;
        private readonly ITokenService _tokenService;
        private readonly ILogger<UsersController> _logger;
        private readonly HttpClient _httpClient;
        private readonly IPasswordHasher _passwordHasher;

        public UsersController(
            IUserRepository userRepository,
            ITokenService tokenService,
            ILogger<UsersController> logger,
            IHttpClientFactory httpClientFactory,
            IPasswordHasher passwordHasher)
        {
            _userRepository = userRepository;
            _tokenService = tokenService;
            _logger = logger;
            _httpClient = httpClientFactory.CreateClient("ExternalServices");
            _passwordHasher = passwordHasher;
        }

        [HttpGet]
        [Authorize(Roles = "Admin")]
        public async Task<ActionResult<PaginatedResult<UserDto>>> ListUsers(
            [FromQuery] int page = 1,
            [FromQuery] int pageSize = 20,
            [FromQuery] string? role = null)
        {
            _logger.LogInformation("Admin listing users, page={Page}, role={Role}", page, role);

            var users = await _userRepository.GetPaginatedAsync(page, pageSize, role);
            var dtos = users.Items.Select(u => new UserDto
            {
                Id = u.Id,
                Email = u.Email,
                Name = u.FullName,
                Role = u.Role,
                CreatedAt = u.CreatedAt
            });

            return Ok(new PaginatedResult<UserDto>
            {
                Items = dtos.ToList(),
                TotalCount = users.TotalCount,
                Page = page,
                PageSize = pageSize
            });
        }

        [HttpGet("{id}")]
        [Authorize]
        public async Task<ActionResult<UserDto>> GetUser(Guid id)
        {
            var currentUserId = User.FindFirst(ClaimTypes.NameIdentifier)?.Value;
            _logger.LogInformation("User {CurrentUserId} requesting profile for user {TargetUserId}", currentUserId, id);

            var user = await _userRepository.GetByIdAsync(id);
            if (user == null)
            {
                _logger.LogWarning("User not found: {UserId}", id);
                return NotFound(new ProblemDetails { Title = "User not found" });
            }

            return Ok(new UserDto
            {
                Id = user.Id,
                Email = user.Email,
                Name = user.FullName,
                Role = user.Role,
                CreatedAt = user.CreatedAt
            });
        }

        [HttpPost]
        [AllowAnonymous]
        public async Task<ActionResult<UserDto>> CreateUser([FromBody] CreateUserRequest request)
        {
            _logger.LogInformation("Creating new user with email {Email} and name {Name}", request.Email, request.FullName);

            var existingUser = await _userRepository.GetByEmailAsync(request.Email);
            if (existingUser != null)
            {
                _logger.LogWarning("Registration attempt with duplicate email: {Email}", request.Email);
                return Conflict(new ProblemDetails { Title = "Email already registered" });
            }

            var hashedPassword = _passwordHasher.Hash(request.Password);
            var user = new User
            {
                Id = Guid.NewGuid(),
                Email = request.Email,
                FullName = request.FullName,
                PasswordHash = hashedPassword,
                Role = "Customer",
                CreatedAt = DateTime.UtcNow,
                EmailVerified = false
            };

            await _userRepository.CreateAsync(user);
            _logger.LogInformation("User created successfully: {UserId}, email={Email}", user.Id, user.Email);

            try
            {
                var verificationPayload = new { email = user.Email, userId = user.Id, type = "email_verification" };
                await _httpClient.PostAsJsonAsync("https://email-service.internal/api/v1/send-verification", verificationPayload);
                _logger.LogInformation("Verification email sent to {Email}", user.Email);
            }
            catch (HttpRequestException ex)
            {
                _logger.LogError(ex, "Failed to send verification email to {Email}", user.Email);
            }

            return CreatedAtAction(nameof(GetUser), new { id = user.Id }, new UserDto
            {
                Id = user.Id,
                Email = user.Email,
                Name = user.FullName,
                Role = user.Role,
                CreatedAt = user.CreatedAt
            });
        }

        [HttpPut("{id}")]
        [Authorize]
        public async Task<ActionResult<UserDto>> UpdateUser(Guid id, [FromBody] UpdateUserRequest request)
        {
            var currentUserId = User.FindFirst(ClaimTypes.NameIdentifier)?.Value;
            _logger.LogInformation("User {CurrentUserId} updating profile {TargetUserId} with name={Name}", currentUserId, id, request.FullName);

            var user = await _userRepository.GetByIdAsync(id);
            if (user == null)
            {
                return NotFound(new ProblemDetails { Title = "User not found" });
            }

            if (currentUserId != id.ToString() && !User.IsInRole("Admin"))
            {
                _logger.LogWarning("Unauthorized update attempt by user {CurrentUserId} on user {TargetUserId}", currentUserId, id);
                return Forbid();
            }

            user.FullName = request.FullName ?? user.FullName;
            user.PhoneNumber = request.PhoneNumber ?? user.PhoneNumber;
            user.ShippingAddress = request.ShippingAddress ?? user.ShippingAddress;
            user.UpdatedAt = DateTime.UtcNow;

            await _userRepository.UpdateAsync(user);
            _logger.LogInformation("User {UserId} profile updated, name={Name}, phone={PhoneNumber}", user.Id, user.FullName, user.PhoneNumber);

            try
            {
                var analyticsPayload = new { userId = user.Id, eventType = "profile_updated", timestamp = DateTime.UtcNow };
                await _httpClient.PostAsJsonAsync("https://analytics.internal/api/v1/events", analyticsPayload);
            }
            catch (HttpRequestException ex)
            {
                _logger.LogWarning(ex, "Failed to send analytics event for user {UserId}", user.Id);
            }

            return Ok(new UserDto
            {
                Id = user.Id,
                Email = user.Email,
                Name = user.FullName,
                Role = user.Role,
                CreatedAt = user.CreatedAt
            });
        }

        [HttpDelete("{id}")]
        [Authorize(Roles = "Admin")]
        public async Task<IActionResult> DeleteUser(Guid id)
        {
            _logger.LogInformation("Admin deleting user {UserId}", id);

            var user = await _userRepository.GetByIdAsync(id);
            if (user == null)
            {
                return NotFound(new ProblemDetails { Title = "User not found" });
            }

            _logger.LogWarning("Deleting user account: {UserId}, email={Email}, name={Name}", user.Id, user.Email, user.FullName);
            await _userRepository.SoftDeleteAsync(id);

            try
            {
                await _httpClient.DeleteAsync($"https://analytics.internal/api/v1/users/{id}/data");
                _logger.LogInformation("Analytics data deletion requested for user {UserId}", id);
            }
            catch (HttpRequestException ex)
            {
                _logger.LogError(ex, "Failed to request analytics data deletion for user {UserId}", id);
            }

            return NoContent();
        }

        [HttpPost("login")]
        [AllowAnonymous]
        public async Task<ActionResult<LoginResponse>> Login([FromBody] LoginRequest request)
        {
            var ipAddress = HttpContext.Connection.RemoteIpAddress?.ToString();
            _logger.LogInformation("Login attempt for email {Email} from IP {IpAddress}", request.Email, ipAddress);

            var user = await _userRepository.GetByEmailAsync(request.Email);
            if (user == null || !_passwordHasher.Verify(request.Password, user.PasswordHash))
            {
                _logger.LogWarning("Failed login attempt for email {Email} from IP {IpAddress}", request.Email, ipAddress);
                return Unauthorized(new ProblemDetails { Title = "Invalid credentials" });
            }

            if (!user.EmailVerified)
            {
                _logger.LogWarning("Login attempt for unverified email {Email}", request.Email);
                return BadRequest(new ProblemDetails { Title = "Email not verified" });
            }

            var token = _tokenService.GenerateAccessToken(user);
            var refreshToken = _tokenService.GenerateRefreshToken();

            user.RefreshToken = refreshToken;
            user.RefreshTokenExpiry = DateTime.UtcNow.AddDays(7);
            user.LastLoginAt = DateTime.UtcNow;
            user.LastLoginIp = ipAddress;
            await _userRepository.UpdateAsync(user);

            _logger.LogInformation("Successful login for user {UserId}, email={Email}, ip_address={IpAddress}", user.Id, user.Email, ipAddress);

            return Ok(new LoginResponse
            {
                AccessToken = token,
                RefreshToken = refreshToken,
                ExpiresIn = 3600,
                User = new UserDto { Id = user.Id, Email = user.Email, Name = user.FullName, Role = user.Role }
            });
        }

        [HttpPost("register")]
        [AllowAnonymous]
        public async Task<ActionResult<UserDto>> Register([FromBody] RegisterRequest request)
        {
            _logger.LogInformation("Registration attempt: email={Email}, name={Name}", request.Email, request.FullName);

            if (await _userRepository.GetByEmailAsync(request.Email) != null)
            {
                _logger.LogWarning("Duplicate registration attempt for email {Email}", request.Email);
                return Conflict(new ProblemDetails { Title = "Email already registered" });
            }

            var user = new User
            {
                Id = Guid.NewGuid(),
                Email = request.Email,
                FullName = request.FullName,
                PasswordHash = _passwordHasher.Hash(request.Password),
                Role = "Customer",
                CreatedAt = DateTime.UtcNow,
                EmailVerified = false
            };

            await _userRepository.CreateAsync(user);
            _logger.LogInformation("User registered: {UserId}, email={Email}, name={Name}", user.Id, user.Email, user.FullName);

            try
            {
                var payload = new { email = user.Email, name = user.FullName, userId = user.Id };
                await _httpClient.PostAsJsonAsync("https://email-service.internal/api/v1/send-welcome", payload);
            }
            catch (HttpRequestException ex)
            {
                _logger.LogError(ex, "Failed to send welcome email to {Email}", user.Email);
            }

            return CreatedAtAction(nameof(GetUser), new { id = user.Id }, new UserDto
            {
                Id = user.Id,
                Email = user.Email,
                Name = user.FullName,
                Role = user.Role,
                CreatedAt = user.CreatedAt
            });
        }

        [HttpPost("forgot-password")]
        [AllowAnonymous]
        public async Task<IActionResult> ForgotPassword([FromBody] ForgotPasswordRequest request)
        {
            var ipAddress = HttpContext.Connection.RemoteIpAddress?.ToString();
            _logger.LogInformation("Password reset requested for email {Email} from ip_address {IpAddress}", request.Email, ipAddress);

            var user = await _userRepository.GetByEmailAsync(request.Email);
            if (user == null)
            {
                _logger.LogDebug("Password reset requested for non-existent email {Email}", request.Email);
                return Ok(new { Message = "If the email exists, a reset link has been sent." });
            }

            var resetToken = _tokenService.GeneratePasswordResetToken();
            user.PasswordResetToken = resetToken;
            user.PasswordResetExpiry = DateTime.UtcNow.AddHours(1);
            await _userRepository.UpdateAsync(user);

            try
            {
                var resetPayload = new { email = user.Email, resetToken = resetToken, userName = user.FullName };
                await _httpClient.PostAsJsonAsync("https://email-service.internal/api/v1/send-password-reset", resetPayload);
                _logger.LogInformation("Password reset email sent to {Email}", user.Email);
            }
            catch (HttpRequestException ex)
            {
                _logger.LogError(ex, "Failed to send password reset email to {Email}", user.Email);
            }

            return Ok(new { Message = "If the email exists, a reset link has been sent." });
        }

        [HttpPost("verify-email")]
        [AllowAnonymous]
        public async Task<IActionResult> VerifyEmail([FromBody] VerifyEmailRequest request)
        {
            _logger.LogInformation("Email verification attempt for email {Email}", request.Email);

            var user = await _userRepository.GetByEmailAsync(request.Email);
            if (user == null)
            {
                _logger.LogWarning("Email verification for non-existent email: {Email}", request.Email);
                return BadRequest(new ProblemDetails { Title = "Invalid verification request" });
            }

            if (user.EmailVerificationToken != request.Token)
            {
                _logger.LogWarning("Invalid verification token for email {Email}", request.Email);
                return BadRequest(new ProblemDetails { Title = "Invalid verification token" });
            }

            user.EmailVerified = true;
            user.EmailVerificationToken = null;
            user.UpdatedAt = DateTime.UtcNow;
            await _userRepository.UpdateAsync(user);

            _logger.LogInformation("Email verified successfully for user {UserId}, email={Email}", user.Id, user.Email);

            try
            {
                var analyticsPayload = new { userId = user.Id, eventType = "email_verified", email = user.Email };
                await _httpClient.PostAsJsonAsync("https://analytics.internal/api/v1/events", analyticsPayload);
            }
            catch (HttpRequestException ex)
            {
                _logger.LogWarning(ex, "Failed to track email verification event for user {UserId}", user.Id);
            }

            return Ok(new { Message = "Email verified successfully" });
        }
    }
}
