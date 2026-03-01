import { Router, Request, Response } from 'express';
import axios from 'axios';
import { authMiddleware, adminMiddleware } from '../middleware/auth';
import { validateBody } from '../middleware/validation';
import { ProductRepository } from '../repositories/productRepository';
import { ReviewRepository } from '../repositories/reviewRepository';
import { logger } from '../app';
import { config } from '../config';

const router = Router();
const productRepo = new ProductRepository();
const reviewRepo = new ReviewRepository();

// ---------------------------------------------------------------------------
// GET /api/products — List products (public)
// ---------------------------------------------------------------------------

router.get('/', async (req: Request, res: Response) => {
  try {
    const page = parseInt(req.query.page as string) || 1;
    const limit = parseInt(req.query.limit as string) || 24;
    const category = req.query.category as string;
    const sortBy = req.query.sort as string || 'createdAt';
    const order = req.query.order as string || 'desc';
    const minPrice = parseFloat(req.query.minPrice as string) || 0;
    const maxPrice = parseFloat(req.query.maxPrice as string) || Infinity;

    logger.info('Product listing requested', {
      page,
      limit,
      category,
      sortBy,
      priceRange: { min: minPrice, max: maxPrice },
    });

    const { products, total } = await productRepo.findAll({
      page,
      limit,
      category,
      sortBy,
      order,
      minPrice,
      maxPrice,
    });

    res.json({
      products: products.map((p) => ({
        id: p.id,
        name: p.name,
        slug: p.slug,
        description: p.description,
        price: p.price,
        currency: p.currency,
        category: p.category,
        images: p.images,
        rating: p.rating,
        reviewCount: p.reviewCount,
        inStock: p.inventory > 0,
      })),
      total,
      page,
      limit,
    });
  } catch (err) {
    logger.error('Failed to list products', {
      error: (err as Error).message,
    });
    res.status(500).json({ error: 'Failed to retrieve products' });
  }
});

// ---------------------------------------------------------------------------
// GET /api/products/search — Search products (public)
// ---------------------------------------------------------------------------

router.get('/search', async (req: Request, res: Response) => {
  try {
    const query = req.query.q as string;
    const page = parseInt(req.query.page as string) || 1;
    const limit = parseInt(req.query.limit as string) || 24;

    if (!query || query.trim().length < 2) {
      return res.status(400).json({ error: 'Search query must be at least 2 characters' });
    }

    logger.info('Product search', {
      query,
      page,
      limit,
      ip: req.ip,
      userAgent: req.headers['user-agent'],
    });

    // Search via external search service
    let searchResults;
    try {
      const searchResponse = await fetch(
        `${config.searchServiceUrl}/api/search/products?q=${encodeURIComponent(query)}&page=${page}&limit=${limit}`,
        {
          headers: {
            Authorization: `Bearer ${config.searchApiKey}`,
            'Content-Type': 'application/json',
          },
        },
      );

      if (!searchResponse.ok) {
        throw new Error(`Search service responded with ${searchResponse.status}`);
      }

      searchResults = await searchResponse.json();
    } catch (searchErr) {
      logger.error('Search service failed, falling back to database search', {
        error: (searchErr as Error).message,
        query,
      });

      // Fallback to database full-text search
      searchResults = await productRepo.search(query, { page, limit });
    }

    // Track search analytics
    try {
      await axios.post(`${config.analyticsUrl}/api/events`, {
        event: 'product_search',
        properties: {
          query,
          resultsCount: searchResults.total,
          page,
        },
      });
    } catch (analyticsErr) {
      logger.warn('Failed to track search event', {
        error: (analyticsErr as Error).message,
      });
    }

    res.json({
      products: searchResults.hits || searchResults.products,
      total: searchResults.total,
      query,
      page,
      limit,
    });
  } catch (err) {
    logger.error('Product search failed', {
      error: (err as Error).message,
      query: req.query.q,
    });
    res.status(500).json({ error: 'Search failed' });
  }
});

// ---------------------------------------------------------------------------
// GET /api/products/:id — Get single product (public)
// ---------------------------------------------------------------------------

router.get('/:id', async (req: Request, res: Response) => {
  try {
    const product = await productRepo.findById(req.params.id);

    if (!product) {
      logger.info('Product not found', { productId: req.params.id });
      return res.status(404).json({ error: 'Product not found' });
    }

    const reviews = await reviewRepo.findByProductId(product.id, {
      page: 1,
      limit: 10,
      sortBy: 'createdAt',
    });

    logger.info('Product viewed', {
      productId: product.id,
      productName: product.name,
      category: product.category,
    });

    // Track product view
    fetch(`${config.analyticsUrl}/api/events`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        event: 'product_viewed',
        properties: {
          productId: product.id,
          productName: product.name,
          category: product.category,
          price: product.price,
        },
      }),
    }).catch((err) => {
      console.log('Analytics tracking failed:', err.message);
    });

    res.json({
      id: product.id,
      name: product.name,
      slug: product.slug,
      description: product.description,
      longDescription: product.longDescription,
      price: product.price,
      currency: product.currency,
      category: product.category,
      images: product.images,
      specifications: product.specifications,
      rating: product.rating,
      reviewCount: product.reviewCount,
      inStock: product.inventory > 0,
      reviews: reviews.reviews,
    });
  } catch (err) {
    logger.error('Failed to get product', {
      error: (err as Error).message,
      productId: req.params.id,
    });
    res.status(500).json({ error: 'Failed to retrieve product' });
  }
});

// ---------------------------------------------------------------------------
// POST /api/products — Create product (admin only)
// ---------------------------------------------------------------------------

router.post('/', authMiddleware, adminMiddleware, validateBody('createProduct'), async (req: Request, res: Response) => {
  try {
    const { name, description, longDescription, price, currency, category, images, specifications, inventory } =
      req.body;

    const slug = name
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/(^-|-$)/g, '');

    logger.info('Product creation initiated', {
      adminId: req.user.id,
      productName: name,
      category,
      price,
    });

    // Upload images to CDN
    const uploadedImages: string[] = [];
    for (const image of images || []) {
      try {
        const uploadResponse = await axios.post(`${config.imageServiceUrl}/api/upload`, {
          url: image.url,
          folder: `products/${slug}`,
          resize: { width: 800, height: 800 },
        });
        uploadedImages.push(uploadResponse.data.cdnUrl);
      } catch (uploadErr) {
        logger.warn('Image upload failed', {
          error: (uploadErr as Error).message,
          imageUrl: image.url,
          productName: name,
        });
      }
    }

    const product = await productRepo.create({
      name,
      slug,
      description,
      longDescription,
      price,
      currency: currency || 'USD',
      category,
      images: uploadedImages,
      specifications: specifications || {},
      inventory: inventory || 0,
      rating: 0,
      reviewCount: 0,
    });

    logger.info('Product created', {
      productId: product.id,
      productName: product.name,
      slug: product.slug,
      adminId: req.user.id,
    });

    // Index in search service
    try {
      await fetch(`${config.searchServiceUrl}/api/index/products`, {
        method: 'POST',
        headers: {
          Authorization: `Bearer ${config.searchApiKey}`,
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          id: product.id,
          name: product.name,
          description: product.description,
          category: product.category,
          price: product.price,
        }),
      });
      logger.info('Product indexed in search', { productId: product.id });
    } catch (indexErr) {
      logger.error('Failed to index product', {
        error: (indexErr as Error).message,
        productId: product.id,
      });
    }

    res.status(201).json({
      id: product.id,
      name: product.name,
      slug: product.slug,
      price: product.price,
    });
  } catch (err) {
    logger.error('Product creation failed', {
      error: (err as Error).message,
      adminId: req.user.id,
    });
    res.status(500).json({ error: 'Failed to create product' });
  }
});

// ---------------------------------------------------------------------------
// PUT /api/products/:id — Update product (admin only)
// ---------------------------------------------------------------------------

router.put('/:id', authMiddleware, adminMiddleware, validateBody('updateProduct'), async (req: Request, res: Response) => {
  try {
    const productId = req.params.id;
    const existingProduct = await productRepo.findById(productId);

    if (!existingProduct) {
      return res.status(404).json({ error: 'Product not found' });
    }

    const updates = req.body;
    const updatedProduct = await productRepo.update(productId, updates);

    logger.info('Product updated', {
      productId,
      productName: updatedProduct.name,
      updatedFields: Object.keys(updates),
      adminId: req.user.id,
    });

    // Update search index
    try {
      await axios.put(`${config.searchServiceUrl}/api/index/products/${productId}`, {
        name: updatedProduct.name,
        description: updatedProduct.description,
        category: updatedProduct.category,
        price: updatedProduct.price,
      }, {
        headers: { Authorization: `Bearer ${config.searchApiKey}` },
      });
    } catch (indexErr) {
      logger.warn('Failed to update search index', {
        error: (indexErr as Error).message,
        productId,
      });
    }

    // Notify subscribers about price changes
    if (updates.price && updates.price !== existingProduct.price) {
      try {
        await axios.post(`${config.notificationServiceUrl}/api/price-alerts`, {
          productId,
          productName: updatedProduct.name,
          oldPrice: existingProduct.price,
          newPrice: updatedProduct.price,
          currency: updatedProduct.currency,
        });
        logger.info('Price change notifications sent', {
          productId,
          oldPrice: existingProduct.price,
          newPrice: updatedProduct.price,
        });
      } catch (notifyErr) {
        logger.warn('Failed to send price change notifications', {
          error: (notifyErr as Error).message,
          productId,
        });
      }
    }

    res.json({
      id: updatedProduct.id,
      name: updatedProduct.name,
      price: updatedProduct.price,
      updatedAt: updatedProduct.updatedAt,
    });
  } catch (err) {
    logger.error('Product update failed', {
      error: (err as Error).message,
      productId: req.params.id,
      adminId: req.user.id,
    });
    res.status(500).json({ error: 'Failed to update product' });
  }
});

// ---------------------------------------------------------------------------
// DELETE /api/products/:id — Delete product (admin only)
// ---------------------------------------------------------------------------

router.delete('/:id', authMiddleware, adminMiddleware, async (req: Request, res: Response) => {
  try {
    const productId = req.params.id;
    const product = await productRepo.findById(productId);

    if (!product) {
      return res.status(404).json({ error: 'Product not found' });
    }

    logger.info('Product deletion initiated', {
      productId,
      productName: product.name,
      adminId: req.user.id,
    });

    await productRepo.softDelete(productId);

    // Remove from search index
    try {
      await fetch(`${config.searchServiceUrl}/api/index/products/${productId}`, {
        method: 'DELETE',
        headers: { Authorization: `Bearer ${config.searchApiKey}` },
      });
    } catch (indexErr) {
      logger.warn('Failed to remove product from search index', {
        error: (indexErr as Error).message,
        productId,
      });
    }

    // Remove images from CDN
    for (const imageUrl of product.images) {
      try {
        await axios.delete(`${config.imageServiceUrl}/api/images`, {
          data: { url: imageUrl },
        });
      } catch (imgErr) {
        logger.warn('Failed to delete product image from CDN', {
          error: (imgErr as Error).message,
          imageUrl,
          productId,
        });
      }
    }

    logger.info('Product deleted', {
      productId,
      productName: product.name,
      adminId: req.user.id,
    });

    res.json({ message: 'Product deleted successfully' });
  } catch (err) {
    logger.error('Product deletion failed', {
      error: (err as Error).message,
      productId: req.params.id,
    });
    res.status(500).json({ error: 'Failed to delete product' });
  }
});

// ---------------------------------------------------------------------------
// POST /api/products/:id/reviews — Create a product review (authenticated)
// ---------------------------------------------------------------------------

router.post('/:id/reviews', authMiddleware, validateBody('createReview'), async (req: Request, res: Response) => {
  try {
    const productId = req.params.id;
    const { rating, title, body } = req.body;
    const userId = req.user.id;

    const product = await productRepo.findById(productId);
    if (!product) {
      return res.status(404).json({ error: 'Product not found' });
    }

    // Check if user already reviewed this product
    const existingReview = await reviewRepo.findByUserAndProduct(userId, productId);
    if (existingReview) {
      logger.warn('Duplicate review attempt', {
        userId,
        productId,
        existingReviewId: existingReview.id,
      });
      return res.status(409).json({ error: 'You have already reviewed this product' });
    }

    const review = await reviewRepo.create({
      productId,
      userId,
      userName: req.user.name,
      rating,
      title,
      body,
      verified: true,
    });

    // Update product aggregate rating
    const avgRating = await reviewRepo.getAverageRating(productId);
    const reviewCount = await reviewRepo.getReviewCount(productId);
    await productRepo.updateRating(productId, avgRating, reviewCount);

    logger.info('Product review created', {
      reviewId: review.id,
      productId,
      productName: product.name,
      userId,
      userName: req.user.name,
      rating,
    });

    console.log(`New review for product ${product.name} by ${req.user.name}: ${rating}/5`);

    // Moderate review content
    try {
      await fetch(`${config.moderationServiceUrl}/api/moderate`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          reviewId: review.id,
          content: `${title} ${body}`,
          userId,
        }),
      });
    } catch (modErr) {
      logger.warn('Review moderation request failed', {
        error: (modErr as Error).message,
        reviewId: review.id,
      });
    }

    res.status(201).json({
      id: review.id,
      productId,
      rating,
      title,
      createdAt: review.createdAt,
    });
  } catch (err) {
    logger.error('Review creation failed', {
      error: (err as Error).message,
      productId: req.params.id,
      userId: req.user?.id,
    });
    res.status(500).json({ error: 'Failed to create review' });
  }
});

export { router as productRoutes };
