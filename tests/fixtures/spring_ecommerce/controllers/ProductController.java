package com.ecommerce.controllers;

import com.ecommerce.dto.CreateProductRequest;
import com.ecommerce.dto.CreateReviewRequest;
import com.ecommerce.dto.ProductResponse;
import com.ecommerce.dto.ReviewResponse;
import com.ecommerce.dto.UpdateProductRequest;
import com.ecommerce.exceptions.ProductNotFoundException;
import com.ecommerce.models.Product;
import com.ecommerce.models.Review;
import com.ecommerce.services.ProductService;
import com.ecommerce.services.ReviewService;
import com.ecommerce.services.SearchIndexService;
import jakarta.validation.Valid;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.http.HttpEntity;
import org.springframework.http.HttpHeaders;
import org.springframework.http.HttpMethod;
import org.springframework.http.HttpStatus;
import org.springframework.http.MediaType;
import org.springframework.http.ResponseEntity;
import org.springframework.security.access.annotation.Secured;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PatchMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.PutMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;
import org.springframework.web.client.RestTemplate;
import org.springframework.web.multipart.MultipartFile;
import org.springframework.web.reactive.function.client.WebClient;

import java.math.BigDecimal;
import java.util.List;
import java.util.Map;
import java.util.UUID;

@RestController
@RequestMapping("/api/v1/products")
public class ProductController {

    private static final Logger logger = LoggerFactory.getLogger(ProductController.class);
    private static final Logger auditLog = LoggerFactory.getLogger("audit.products");

    private final ProductService productService;
    private final ReviewService reviewService;
    private final RestTemplate restTemplate;
    private final WebClient webClient;

    @Value("${services.search.url}")
    private String searchServiceUrl;

    @Value("${services.image.url}")
    private String imageServiceUrl;

    @Value("${services.recommendation.url}")
    private String recommendationServiceUrl;

    public ProductController(
            ProductService productService,
            ReviewService reviewService,
            RestTemplate restTemplate,
            WebClient.Builder webClientBuilder) {
        this.productService = productService;
        this.reviewService = reviewService;
        this.restTemplate = restTemplate;
        this.webClient = webClientBuilder.baseUrl("https://internal.ecommerce.com").build();
    }

    @GetMapping
    public ResponseEntity<Page<ProductResponse>> listProducts(
            @RequestParam(required = false) String category,
            @RequestParam(required = false) BigDecimal minPrice,
            @RequestParam(required = false) BigDecimal maxPrice,
            @RequestParam(defaultValue = "relevance") String sortBy,
            Pageable pageable) {
        logger.info("Listing products — category: {}, price_range: [{}, {}], sort: {}",
                category, minPrice, maxPrice, sortBy);
        Page<ProductResponse> products = productService.findAll(category, minPrice, maxPrice, sortBy, pageable);
        logger.info("Returned {} products out of {} total", products.getNumberOfElements(), products.getTotalElements());
        return ResponseEntity.ok(products);
    }

    @GetMapping("/{id}")
    public ResponseEntity<ProductResponse> getProduct(@PathVariable UUID id) {
        logger.info("Fetching product: {}", id);
        Product product = productService.findById(id)
                .orElseThrow(() -> new ProductNotFoundException("Product not found: " + id));

        // Track view for recommendation engine (fire-and-forget)
        webClient.post()
                .uri(recommendationServiceUrl + "/api/v1/events/view")
                .contentType(MediaType.APPLICATION_JSON)
                .bodyValue(Map.of(
                        "product_id", id.toString(),
                        "category", product.getCategory(),
                        "timestamp", System.currentTimeMillis()
                ))
                .retrieve()
                .bodyToMono(Void.class)
                .subscribe(
                        v -> {},
                        err -> logger.warn("Failed to track product view for: {}", id)
                );

        logger.info("Product {} retrieved — name: {}, price: {}", id, product.getName(), product.getPrice());
        return ResponseEntity.ok(ProductResponse.fromEntity(product));
    }

    @PostMapping
    @PreAuthorize("hasRole('ADMIN')")
    public ResponseEntity<ProductResponse> createProduct(@Valid @RequestBody CreateProductRequest request) {
        logger.info("Creating product — name: {}, category: {}, price: {}",
                request.getName(), request.getCategory(), request.getPrice());

        Product product = productService.create(request);
        auditLog.info("Product created — id: {}, name: {}, sku: {}", product.getId(), product.getName(), product.getSku());

        // Index in search service
        try {
            HttpHeaders headers = new HttpHeaders();
            headers.setContentType(MediaType.APPLICATION_JSON);
            HttpEntity<Map<String, Object>> indexRequest = new HttpEntity<>(
                    Map.of(
                            "id", product.getId().toString(),
                            "name", product.getName(),
                            "description", product.getDescription(),
                            "category", product.getCategory(),
                            "price", product.getPrice(),
                            "tags", product.getTags()
                    ),
                    headers
            );
            restTemplate.exchange(
                    searchServiceUrl + "/api/v1/index/products",
                    HttpMethod.POST,
                    indexRequest,
                    Void.class
            );
            logger.info("Product {} indexed in search service", product.getId());
        } catch (Exception e) {
            logger.error("Failed to index product {} in search service: {}", product.getId(), e.getMessage());
        }

        return ResponseEntity.status(HttpStatus.CREATED).body(ProductResponse.fromEntity(product));
    }

    @PutMapping("/{id}")
    @PreAuthorize("hasRole('ADMIN')")
    public ResponseEntity<ProductResponse> updateProduct(
            @PathVariable UUID id,
            @Valid @RequestBody UpdateProductRequest request) {
        logger.info("Updating product {} — name: {}, price: {}", id, request.getName(), request.getPrice());

        Product product = productService.update(id, request);
        auditLog.info("Product updated — id: {}, changes: name={}, price={}", id, request.getName(), request.getPrice());

        // Update search index
        try {
            restTemplate.put(
                    searchServiceUrl + "/api/v1/index/products/" + id,
                    Map.of(
                            "name", product.getName(),
                            "description", product.getDescription(),
                            "category", product.getCategory(),
                            "price", product.getPrice()
                    )
            );
            logger.info("Search index updated for product: {}", id);
        } catch (Exception e) {
            logger.warn("Failed to update search index for product {}: {}", id, e.getMessage());
        }

        return ResponseEntity.ok(ProductResponse.fromEntity(product));
    }

    @DeleteMapping("/{id}")
    @PreAuthorize("hasRole('ADMIN')")
    public ResponseEntity<Void> deleteProduct(@PathVariable UUID id) {
        logger.info("Deleting product: {}", id);
        Product product = productService.findById(id)
                .orElseThrow(() -> new ProductNotFoundException("Product not found: " + id));

        productService.delete(id);
        auditLog.warn("Product deleted — id: {}, name: {}, sku: {}", id, product.getName(), product.getSku());

        // Remove from search index
        try {
            restTemplate.delete(searchServiceUrl + "/api/v1/index/products/" + id);
            logger.info("Product {} removed from search index", id);
        } catch (Exception e) {
            logger.error("Failed to remove product {} from search index: {}", id, e.getMessage());
        }

        // Remove associated images
        try {
            restTemplate.delete(imageServiceUrl + "/api/v1/images/product/" + id);
        } catch (Exception e) {
            logger.warn("Failed to clean up images for product {}: {}", id, e.getMessage());
        }

        return ResponseEntity.noContent().build();
    }

    @GetMapping("/search")
    public ResponseEntity<Page<ProductResponse>> searchProducts(
            @RequestParam String q,
            @RequestParam(required = false) String category,
            @RequestParam(required = false) BigDecimal minPrice,
            @RequestParam(required = false) BigDecimal maxPrice,
            Pageable pageable) {
        logger.info("Product search — query: '{}', category: {}, price_range: [{}, {}]",
                q, category, minPrice, maxPrice);

        // Delegate to search service for full-text search
        String searchUrl = String.format(
                "%s/api/v1/search?q=%s&category=%s&min_price=%s&max_price=%s&page=%d&size=%d",
                searchServiceUrl, q, category, minPrice, maxPrice,
                pageable.getPageNumber(), pageable.getPageSize()
        );

        try {
            ResponseEntity<Map> searchResponse = restTemplate.getForEntity(searchUrl, Map.class);
            List<String> productIds = (List<String>) searchResponse.getBody().get("ids");
            logger.info("Search returned {} results for query: '{}'", productIds.size(), q);

            Page<ProductResponse> results = productService.findByIds(productIds, pageable);
            return ResponseEntity.ok(results);
        } catch (Exception e) {
            logger.error("Search service failed for query '{}': {}", q, e.getMessage());
            // Fallback to database search
            Page<ProductResponse> fallback = productService.searchFallback(q, category, pageable);
            return ResponseEntity.ok(fallback);
        }
    }

    @PostMapping("/{id}/reviews")
    @Secured("ROLE_USER")
    public ResponseEntity<ReviewResponse> addReview(
            @PathVariable UUID id,
            @Valid @RequestBody CreateReviewRequest request) {
        logger.info("New review for product {} — rating: {}, author_email: {}",
                id, request.getRating(), request.getAuthorEmail());

        Product product = productService.findById(id)
                .orElseThrow(() -> new ProductNotFoundException("Product not found: " + id));

        Review review = reviewService.create(id, request);
        logger.info("Review {} created for product {} by user email: {} (name: {})",
                review.getId(), id, request.getAuthorEmail(), request.getAuthorName());

        // Update product rating aggregate
        productService.recalculateRating(id);

        // Update search index with new rating
        try {
            restTemplate.postForObject(
                    searchServiceUrl + "/api/v1/index/products/" + id + "/rating",
                    Map.of("average_rating", product.getAverageRating(), "review_count", product.getReviewCount()),
                    Void.class
            );
        } catch (Exception e) {
            logger.warn("Failed to update search index rating for product {}: {}", id, e.getMessage());
        }

        return ResponseEntity.status(HttpStatus.CREATED).body(ReviewResponse.fromEntity(review));
    }

    @PatchMapping("/{id}/inventory")
    @PreAuthorize("hasRole('ADMIN')")
    public ResponseEntity<ProductResponse> updateInventory(
            @PathVariable UUID id,
            @RequestBody Map<String, Integer> inventoryUpdate) {
        int newQuantity = inventoryUpdate.getOrDefault("quantity", 0);
        logger.info("Inventory update for product {} — new quantity: {}", id, newQuantity);

        Product product = productService.updateInventory(id, newQuantity);

        if (newQuantity <= 5) {
            logger.warn("Low inventory alert — product: {} ({}), quantity: {}", id, product.getName(), newQuantity);

            // Notify inventory management
            webClient.post()
                    .uri("/api/v1/alerts/low-inventory")
                    .bodyValue(Map.of(
                            "product_id", id.toString(),
                            "product_name", product.getName(),
                            "sku", product.getSku(),
                            "current_quantity", newQuantity
                    ))
                    .retrieve()
                    .bodyToMono(Void.class)
                    .subscribe();
        }

        return ResponseEntity.ok(ProductResponse.fromEntity(product));
    }

    @PostMapping("/{id}/images")
    @PreAuthorize("hasRole('ADMIN')")
    public ResponseEntity<Map<String, String>> uploadProductImage(
            @PathVariable UUID id,
            @RequestParam("file") MultipartFile file) {
        logger.info("Image upload for product {} — filename: {}, size: {} bytes",
                id, file.getOriginalFilename(), file.getSize());

        try {
            HttpHeaders headers = new HttpHeaders();
            headers.setContentType(MediaType.MULTIPART_FORM_DATA);

            String imageUrl = restTemplate.postForObject(
                    imageServiceUrl + "/api/v1/images/upload?product_id=" + id,
                    file.getBytes(),
                    String.class
            );

            logger.info("Image uploaded for product {} — url: {}", id, imageUrl);
            return ResponseEntity.ok(Map.of("url", imageUrl));
        } catch (Exception e) {
            logger.error("Image upload failed for product {}: {}", id, e.getMessage());
            return ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR)
                    .body(Map.of("error", "Image upload failed"));
        }
    }
}
