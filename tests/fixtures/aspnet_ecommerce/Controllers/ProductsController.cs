using System;
using System.Collections.Generic;
using System.Linq;
using System.Net.Http;
using System.Net.Http.Json;
using System.Security.Claims;
using System.Threading.Tasks;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Caching.Distributed;
using EcommerceApi.Models;
using EcommerceApi.Services;
using EcommerceApi.DTOs;

namespace EcommerceApi.Controllers
{
    [ApiController]
    [Route("api/v1/[controller]")]
    public class ProductsController : ControllerBase
    {
        private readonly IProductRepository _productRepository;
        private readonly IReviewRepository _reviewRepository;
        private readonly ILogger<ProductsController> _logger;
        private readonly HttpClient _httpClient;
        private readonly IDistributedCache _cache;

        public ProductsController(
            IProductRepository productRepository,
            IReviewRepository reviewRepository,
            ILogger<ProductsController> logger,
            IHttpClientFactory httpClientFactory,
            IDistributedCache cache)
        {
            _productRepository = productRepository;
            _reviewRepository = reviewRepository;
            _logger = logger;
            _httpClient = httpClientFactory.CreateClient("InternalServices");
            _cache = cache;
        }

        [HttpGet]
        [AllowAnonymous]
        public async Task<ActionResult<PaginatedResult<ProductDto>>> ListProducts(
            [FromQuery] int page = 1,
            [FromQuery] int pageSize = 24,
            [FromQuery] string? category = null,
            [FromQuery] decimal? minPrice = null,
            [FromQuery] decimal? maxPrice = null,
            [FromQuery] string sortBy = "created_at")
        {
            _logger.LogInformation(
                "Product listing: page={Page}, category={Category}, priceRange=[{MinPrice}-{MaxPrice}], sort={SortBy}",
                page, category, minPrice, maxPrice, sortBy);

            var products = await _productRepository.GetPaginatedAsync(
                page, pageSize, category, minPrice, maxPrice, sortBy);

            var dtos = products.Items.Select(MapToDto).ToList();

            return Ok(new PaginatedResult<ProductDto>
            {
                Items = dtos,
                TotalCount = products.TotalCount,
                Page = page,
                PageSize = pageSize
            });
        }

        [HttpGet("{id}")]
        [AllowAnonymous]
        public async Task<ActionResult<ProductDetailDto>> GetProduct(Guid id)
        {
            _logger.LogInformation("Product detail request: productId={ProductId}", id);

            var product = await _productRepository.GetByIdWithReviewsAsync(id);
            if (product == null)
            {
                _logger.LogWarning("Product not found: productId={ProductId}", id);
                return NotFound(new ProblemDetails { Title = "Product not found" });
            }

            var averageRating = product.Reviews.Any()
                ? product.Reviews.Average(r => r.Rating)
                : 0.0;

            return Ok(new ProductDetailDto
            {
                Id = product.Id,
                Name = product.Name,
                Description = product.Description,
                Price = product.Price,
                CompareAtPrice = product.CompareAtPrice,
                Sku = product.Sku,
                Category = product.Category,
                Tags = product.Tags,
                Images = product.Images.Select(i => i.Url).ToList(),
                StockQuantity = product.StockQuantity,
                IsAvailable = product.StockQuantity > 0 && product.IsActive,
                AverageRating = averageRating,
                ReviewCount = product.Reviews.Count,
                CreatedAt = product.CreatedAt
            });
        }

        [HttpPost]
        [Authorize(Roles = "Admin")]
        public async Task<ActionResult<ProductDto>> CreateProduct([FromBody] CreateProductRequest request)
        {
            _logger.LogInformation("Product creation: name={Name}, sku={Sku}, price={Price}, category={Category}",
                request.Name, request.Sku, request.Price, request.Category);

            var existingProduct = await _productRepository.GetBySkuAsync(request.Sku);
            if (existingProduct != null)
            {
                _logger.LogWarning("Duplicate SKU on product creation: sku={Sku}", request.Sku);
                return Conflict(new ProblemDetails { Title = "A product with this SKU already exists" });
            }

            var product = new Product
            {
                Id = Guid.NewGuid(),
                Name = request.Name,
                Description = request.Description,
                Price = request.Price,
                CompareAtPrice = request.CompareAtPrice,
                Sku = request.Sku,
                Category = request.Category,
                Tags = request.Tags ?? new List<string>(),
                StockQuantity = request.InitialStock ?? 0,
                IsActive = true,
                CreatedAt = DateTime.UtcNow
            };

            await _productRepository.CreateAsync(product);
            _logger.LogInformation("Product created: productId={ProductId}, name={Name}, sku={Sku}", product.Id, product.Name, product.Sku);

            // Index in search service
            try
            {
                var indexPayload = new
                {
                    id = product.Id,
                    name = product.Name,
                    description = product.Description,
                    category = product.Category,
                    tags = product.Tags,
                    price = product.Price
                };
                await _httpClient.PostAsJsonAsync("https://search-indexer.internal/api/v1/products/index", indexPayload);
                _logger.LogInformation("Product indexed in search service: productId={ProductId}", product.Id);
            }
            catch (HttpRequestException ex)
            {
                _logger.LogError(ex, "Failed to index product in search service: productId={ProductId}", product.Id);
            }

            // Upload images to CDN if provided
            if (request.ImageUrls?.Any() == true)
            {
                try
                {
                    var cdnPayload = new { productId = product.Id, urls = request.ImageUrls };
                    await _httpClient.PostAsJsonAsync("https://cdn-manager.internal/api/v1/images/process", cdnPayload);
                    _logger.LogInformation("CDN image processing queued: productId={ProductId}, imageCount={Count}",
                        product.Id, request.ImageUrls.Count);
                }
                catch (HttpRequestException ex)
                {
                    _logger.LogWarning(ex, "Failed to queue CDN image processing: productId={ProductId}", product.Id);
                }
            }

            return CreatedAtAction(nameof(GetProduct), new { id = product.Id }, MapToDto(product));
        }

        [HttpPut("{id}")]
        [Authorize(Roles = "Admin")]
        public async Task<ActionResult<ProductDto>> UpdateProduct(Guid id, [FromBody] UpdateProductRequest request)
        {
            _logger.LogInformation("Product update: productId={ProductId}, name={Name}, price={Price}", id, request.Name, request.Price);

            var product = await _productRepository.GetByIdAsync(id);
            if (product == null)
            {
                return NotFound(new ProblemDetails { Title = "Product not found" });
            }

            var previousPrice = product.Price;
            product.Name = request.Name ?? product.Name;
            product.Description = request.Description ?? product.Description;
            product.Price = request.Price ?? product.Price;
            product.CompareAtPrice = request.CompareAtPrice ?? product.CompareAtPrice;
            product.Category = request.Category ?? product.Category;
            product.Tags = request.Tags ?? product.Tags;
            product.IsActive = request.IsActive ?? product.IsActive;
            product.UpdatedAt = DateTime.UtcNow;

            await _productRepository.UpdateAsync(product);
            _logger.LogInformation("Product updated: productId={ProductId}, previousPrice={PreviousPrice}, newPrice={NewPrice}",
                product.Id, previousPrice, product.Price);

            // Re-index in search service
            try
            {
                var indexPayload = new
                {
                    id = product.Id,
                    name = product.Name,
                    description = product.Description,
                    category = product.Category,
                    tags = product.Tags,
                    price = product.Price
                };
                await _httpClient.PutAsJsonAsync($"https://search-indexer.internal/api/v1/products/{product.Id}", indexPayload);
                _logger.LogInformation("Product re-indexed: productId={ProductId}", product.Id);
            }
            catch (HttpRequestException ex)
            {
                _logger.LogWarning(ex, "Failed to re-index product: productId={ProductId}", product.Id);
            }

            // Invalidate cache
            await _cache.RemoveAsync($"product:{id}");

            return Ok(MapToDto(product));
        }

        [HttpDelete("{id}")]
        [Authorize(Roles = "Admin")]
        public async Task<IActionResult> DeleteProduct(Guid id)
        {
            _logger.LogInformation("Product deletion: productId={ProductId}", id);

            var product = await _productRepository.GetByIdAsync(id);
            if (product == null)
            {
                return NotFound(new ProblemDetails { Title = "Product not found" });
            }

            await _productRepository.SoftDeleteAsync(id);
            _logger.LogInformation("Product soft-deleted: productId={ProductId}, name={Name}, sku={Sku}", product.Id, product.Name, product.Sku);

            // Remove from search index
            try
            {
                await _httpClient.DeleteAsync($"https://search-indexer.internal/api/v1/products/{id}");
                _logger.LogInformation("Product removed from search index: productId={ProductId}", id);
            }
            catch (HttpRequestException ex)
            {
                _logger.LogWarning(ex, "Failed to remove product from search index: productId={ProductId}", id);
            }

            // Clean up CDN images
            try
            {
                await _httpClient.DeleteAsync($"https://cdn-manager.internal/api/v1/images/product/{id}");
                _logger.LogInformation("CDN image cleanup queued: productId={ProductId}", id);
            }
            catch (HttpRequestException ex)
            {
                _logger.LogWarning(ex, "Failed to queue CDN image cleanup: productId={ProductId}", id);
            }

            await _cache.RemoveAsync($"product:{id}");

            return NoContent();
        }

        [HttpGet("search")]
        [AllowAnonymous]
        public async Task<ActionResult<SearchResultDto>> SearchProducts(
            [FromQuery] string q,
            [FromQuery] int page = 1,
            [FromQuery] int pageSize = 24,
            [FromQuery] string? category = null)
        {
            _logger.LogInformation("Product search: query={Query}, page={Page}, category={Category}", q, page, category);

            if (string.IsNullOrWhiteSpace(q) || q.Length < 2)
            {
                return BadRequest(new ProblemDetails { Title = "Search query must be at least 2 characters" });
            }

            try
            {
                var searchResponse = await _httpClient.GetAsync(
                    $"https://search-indexer.internal/api/v1/products/search?q={Uri.EscapeDataString(q)}&page={page}&pageSize={pageSize}&category={category}");

                if (!searchResponse.IsSuccessStatusCode)
                {
                    _logger.LogError("Search service returned error: statusCode={StatusCode}, query={Query}",
                        searchResponse.StatusCode, q);
                    // Fallback to database search
                    var dbResults = await _productRepository.SearchAsync(q, page, pageSize, category);
                    return Ok(new SearchResultDto
                    {
                        Items = dbResults.Items.Select(MapToDto).ToList(),
                        TotalCount = dbResults.TotalCount,
                        Query = q,
                        Page = page,
                        PageSize = pageSize
                    });
                }

                var searchResult = await searchResponse.Content.ReadFromJsonAsync<SearchResultDto>();
                _logger.LogInformation("Search completed: query={Query}, results={Count}", q, searchResult?.TotalCount);
                return Ok(searchResult);
            }
            catch (HttpRequestException ex)
            {
                _logger.LogError(ex, "Search service unavailable, falling back to database: query={Query}", q);
                var fallbackResults = await _productRepository.SearchAsync(q, page, pageSize, category);
                return Ok(new SearchResultDto
                {
                    Items = fallbackResults.Items.Select(MapToDto).ToList(),
                    TotalCount = fallbackResults.TotalCount,
                    Query = q,
                    Page = page,
                    PageSize = pageSize
                });
            }
        }

        [HttpPost("{id}/reviews")]
        [Authorize]
        public async Task<ActionResult<ReviewDto>> CreateReview(Guid id, [FromBody] CreateReviewRequest request)
        {
            var userId = User.FindFirst(ClaimTypes.NameIdentifier)?.Value;
            var userName = User.FindFirst(ClaimTypes.Name)?.Value;
            _logger.LogInformation("Review submission: productId={ProductId}, userId={UserId}, name={Name}, rating={Rating}",
                id, userId, userName, request.Rating);

            var product = await _productRepository.GetByIdAsync(id);
            if (product == null)
            {
                return NotFound(new ProblemDetails { Title = "Product not found" });
            }

            var existingReview = await _reviewRepository.GetByUserAndProductAsync(Guid.Parse(userId), id);
            if (existingReview != null)
            {
                _logger.LogWarning("Duplicate review attempt: productId={ProductId}, userId={UserId}", id, userId);
                return Conflict(new ProblemDetails { Title = "You have already reviewed this product" });
            }

            if (request.Rating < 1 || request.Rating > 5)
            {
                return BadRequest(new ProblemDetails { Title = "Rating must be between 1 and 5" });
            }

            var review = new Review
            {
                Id = Guid.NewGuid(),
                ProductId = id,
                UserId = Guid.Parse(userId),
                UserName = userName,
                Rating = request.Rating,
                Title = request.Title,
                Body = request.Body,
                CreatedAt = DateTime.UtcNow,
                IsVerifiedPurchase = await _productRepository.HasUserPurchasedAsync(Guid.Parse(userId), id)
            };

            await _reviewRepository.CreateAsync(review);
            _logger.LogInformation("Review created: reviewId={ReviewId}, productId={ProductId}, rating={Rating}",
                review.Id, id, request.Rating);

            await _cache.RemoveAsync($"product:{id}");

            return CreatedAtAction(nameof(GetProduct), new { id = id }, new ReviewDto
            {
                Id = review.Id,
                Rating = review.Rating,
                Title = review.Title,
                Body = review.Body,
                UserName = review.UserName,
                IsVerifiedPurchase = review.IsVerifiedPurchase,
                CreatedAt = review.CreatedAt
            });
        }

        [HttpPatch("{id}/stock")]
        [Authorize(Roles = "Warehouse")]
        public async Task<ActionResult<ProductDto>> UpdateStock(Guid id, [FromBody] UpdateStockRequest request)
        {
            _logger.LogInformation("Stock update: productId={ProductId}, quantity={Quantity}, operation={Operation}",
                id, request.Quantity, request.Operation);

            var product = await _productRepository.GetByIdAsync(id);
            if (product == null)
            {
                return NotFound(new ProblemDetails { Title = "Product not found" });
            }

            var previousStock = product.StockQuantity;

            switch (request.Operation)
            {
                case StockOperation.Set:
                    product.StockQuantity = request.Quantity;
                    break;
                case StockOperation.Increment:
                    product.StockQuantity += request.Quantity;
                    break;
                case StockOperation.Decrement:
                    if (product.StockQuantity < request.Quantity)
                    {
                        _logger.LogWarning("Insufficient stock for decrement: productId={ProductId}, current={Current}, requested={Requested}",
                            id, product.StockQuantity, request.Quantity);
                        return BadRequest(new ProblemDetails { Title = "Insufficient stock" });
                    }
                    product.StockQuantity -= request.Quantity;
                    break;
                default:
                    return BadRequest(new ProblemDetails { Title = "Invalid stock operation" });
            }

            product.UpdatedAt = DateTime.UtcNow;
            await _productRepository.UpdateAsync(product);

            _logger.LogInformation("Stock updated: productId={ProductId}, sku={Sku}, previousStock={Previous}, newStock={New}",
                product.Id, product.Sku, previousStock, product.StockQuantity);

            if (product.StockQuantity == 0)
            {
                _logger.LogWarning("Product out of stock: productId={ProductId}, sku={Sku}, name={Name}", product.Id, product.Sku, product.Name);
            }

            await _cache.RemoveAsync($"product:{id}");

            return Ok(MapToDto(product));
        }

        private static ProductDto MapToDto(Product product)
        {
            return new ProductDto
            {
                Id = product.Id,
                Name = product.Name,
                Price = product.Price,
                Category = product.Category,
                Sku = product.Sku,
                StockQuantity = product.StockQuantity,
                IsAvailable = product.StockQuantity > 0 && product.IsActive,
                CreatedAt = product.CreatedAt
            };
        }
    }
}
