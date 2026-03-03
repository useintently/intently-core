"""
Shop API — FastAPI application entry point.
"""

import logging
import time

from fastapi import FastAPI, Request, Depends
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse

from .routes import router as api_router

logger = logging.getLogger("shop.api")

app = FastAPI(
    title="Shop API",
    version="1.0.0",
)

app.add_middleware(
    CORSMiddleware,
    allow_origins=["https://shop.example.com"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


@app.middleware("http")
async def request_logging(request: Request, call_next):
    start = time.monotonic()
    response = await call_next(request)
    duration = time.monotonic() - start
    logger.info(
        "request completed: method=%s path=%s status=%d duration=%.3fs ip=%s",
        request.method,
        request.url.path,
        response.status_code,
        duration,
        request.client.host if request.client else "unknown",
    )
    return response


@app.get("/health")
async def health():
    return {"status": "healthy", "service": "shop-api"}


@app.get("/health/ready")
async def readiness():
    logger.info("Readiness check requested")
    return {"status": "ready"}


app.include_router(api_router, prefix="/api/v1")
