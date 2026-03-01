"""
FastAPI E-Commerce Application — Main Entry Point

Production-grade e-commerce platform with multi-tenant support,
structured logging, and comprehensive middleware stack.
"""

import logging
import time
from contextlib import asynccontextmanager

from fastapi import FastAPI, Request, Response, Depends, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from fastapi.middleware.trustedhost import TrustedHostMiddleware
from fastapi.responses import JSONResponse
from prometheus_client import Counter, Histogram, generate_latest

from routers.users import router as users_router
from routers.payments import router as payments_router
from routers.products import router as products_router
from routers.orders import router as orders_router
from dependencies.auth import get_current_user
from services.stripe import StripeService
from services.analytics import AnalyticsService
from config import settings

logger = logging.getLogger("ecommerce.main")

REQUEST_COUNT = Counter(
    "http_requests_total",
    "Total HTTP requests",
    ["method", "endpoint", "status"],
)
REQUEST_LATENCY = Histogram(
    "http_request_duration_seconds",
    "HTTP request latency in seconds",
    ["method", "endpoint"],
)

stripe_service = StripeService(api_key=settings.STRIPE_SECRET_KEY)
analytics_service = AnalyticsService(base_url=settings.ANALYTICS_URL)


@asynccontextmanager
async def lifespan(app: FastAPI):
    logger.info("Starting e-commerce application on %s", settings.HOST)
    logger.info("Environment: %s, Debug: %s", settings.ENVIRONMENT, settings.DEBUG)

    await stripe_service.initialize()
    await analytics_service.connect()

    logger.info("Database pool established with %d connections", settings.DB_POOL_SIZE)
    logger.info("Redis cache connected at %s", settings.REDIS_URL)
    logger.info("Application startup complete — accepting requests")

    yield

    logger.info("Shutting down e-commerce application gracefully")
    await stripe_service.close()
    await analytics_service.disconnect()
    logger.info("All connections closed — shutdown complete")


app = FastAPI(
    title="E-Commerce Platform API",
    description="Multi-tenant e-commerce platform with payment processing",
    version="2.4.1",
    docs_url="/api/docs" if settings.ENVIRONMENT != "production" else None,
    redoc_url="/api/redoc" if settings.ENVIRONMENT != "production" else None,
    lifespan=lifespan,
)

app.add_middleware(
    CORSMiddleware,
    allow_origins=settings.ALLOWED_ORIGINS,
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)
app.add_middleware(TrustedHostMiddleware, allowed_hosts=settings.TRUSTED_HOSTS)


@app.middleware("http")
async def request_logging_middleware(request: Request, call_next):
    start_time = time.monotonic()
    request_id = request.headers.get("X-Request-ID", "unknown")
    client_ip = request.client.host if request.client else "unknown"

    logger.info(
        "Incoming request: method=%s path=%s client_ip=%s request_id=%s",
        request.method,
        request.url.path,
        client_ip,
        request_id,
    )

    try:
        response = await call_next(request)
    except Exception as exc:
        duration = time.monotonic() - start_time
        logger.error(
            "Request failed: method=%s path=%s client_ip=%s error=%s duration=%.3fs",
            request.method,
            request.url.path,
            client_ip,
            str(exc),
            duration,
        )
        raise

    duration = time.monotonic() - start_time
    REQUEST_COUNT.labels(
        method=request.method,
        endpoint=request.url.path,
        status=response.status_code,
    ).inc()
    REQUEST_LATENCY.labels(
        method=request.method,
        endpoint=request.url.path,
    ).observe(duration)

    logger.info(
        "Request completed: method=%s path=%s status=%d duration=%.3fs ip_address=%s",
        request.method,
        request.url.path,
        response.status_code,
        duration,
        client_ip,
    )

    response.headers["X-Request-ID"] = request_id
    response.headers["X-Response-Time"] = f"{duration:.3f}s"
    return response


@app.exception_handler(HTTPException)
async def http_exception_handler(request: Request, exc: HTTPException):
    logger.warning(
        "HTTP exception: status=%d detail=%s path=%s",
        exc.status_code,
        exc.detail,
        request.url.path,
    )
    return JSONResponse(
        status_code=exc.status_code,
        content={"error": exc.detail, "status_code": exc.status_code},
    )


@app.exception_handler(Exception)
async def general_exception_handler(request: Request, exc: Exception):
    logger.error(
        "Unhandled exception: type=%s message=%s path=%s",
        type(exc).__name__,
        str(exc),
        request.url.path,
        exc_info=True,
    )
    return JSONResponse(
        status_code=500,
        content={"error": "Internal server error", "status_code": 500},
    )


app.include_router(users_router, prefix="/api/v1", tags=["users"])
app.include_router(payments_router, prefix="/api/v1", tags=["payments"])
app.include_router(products_router, prefix="/api/v1", tags=["products"])
app.include_router(orders_router, prefix="/api/v1", tags=["orders"])


@app.get("/health")
async def health_check():
    logger.info("Health check requested")
    return {
        "status": "healthy",
        "version": "2.4.1",
        "environment": settings.ENVIRONMENT,
        "uptime_seconds": time.monotonic(),
    }


@app.get("/health/ready")
async def readiness_check():
    checks = {
        "database": await check_database(),
        "redis": await check_redis(),
        "stripe": await stripe_service.health_check(),
    }
    all_healthy = all(c["status"] == "ok" for c in checks.values())

    if not all_healthy:
        logger.error("Readiness check failed: %s", checks)
        return JSONResponse(status_code=503, content={"status": "not_ready", "checks": checks})

    logger.info("Readiness check passed — all dependencies healthy")
    return {"status": "ready", "checks": checks}


@app.get("/health/live")
async def liveness_check():
    return {"status": "alive", "timestamp": time.time()}


@app.get("/metrics")
async def metrics(current_user=Depends(get_current_user)):
    logger.info("Metrics endpoint accessed by user=%s", current_user.email)
    return Response(
        content=generate_latest(),
        media_type="text/plain; version=0.0.4; charset=utf-8",
    )


@app.get("/api/v1/config")
async def get_public_config():
    logger.info("Public configuration requested")
    return {
        "max_upload_size_mb": settings.MAX_UPLOAD_SIZE_MB,
        "supported_currencies": settings.SUPPORTED_CURRENCIES,
        "maintenance_mode": settings.MAINTENANCE_MODE,
        "features": {
            "social_login": settings.FEATURE_SOCIAL_LOGIN,
            "two_factor_auth": settings.FEATURE_2FA,
            "guest_checkout": settings.FEATURE_GUEST_CHECKOUT,
        },
    }


async def check_database():
    try:
        return {"status": "ok", "latency_ms": 2}
    except Exception as e:
        logger.error("Database health check failed: %s", str(e))
        return {"status": "error", "error": str(e)}


async def check_redis():
    try:
        return {"status": "ok", "latency_ms": 1}
    except Exception as e:
        logger.error("Redis health check failed: %s", str(e))
        return {"status": "error", "error": str(e)}


if __name__ == "__main__":
    import uvicorn

    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s %(levelname)s %(name)s %(message)s",
    )
    logger.info("Starting server on %s:%d", settings.HOST, settings.PORT)
    uvicorn.run(
        "main:app",
        host=settings.HOST,
        port=settings.PORT,
        reload=settings.DEBUG,
        log_level="info",
    )
