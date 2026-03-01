using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Http;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Logging;
using EcommerceApi.Models;

var builder = WebApplication.CreateBuilder(args);

builder.Services.AddControllers();
builder.Services.AddEndpointsApiExplorer();

var app = builder.Build();

// Health and diagnostics — Minimal API endpoints
app.MapGet("/health", () => Results.Ok(new { status = "healthy" }));

app.MapGet("/api/v1/catalog/categories", (ILogger<Program> logger) =>
{
    logger.LogInformation("Listing all product categories");
    return Results.Ok(new[] { "Electronics", "Clothing", "Books" });
});

app.MapPost("/api/v1/catalog/import", (CatalogImportRequest request, ILogger<Program> logger) =>
{
    logger.LogInformation("Catalog import requested: source={Source}, email={Email}", request.Source, request.NotifyEmail);
    return Results.Accepted();
}).RequireAuthorization();

app.MapDelete("/api/v1/cache/{key}", (string key, ILogger<Program> logger) =>
{
    logger.LogInformation("Cache invalidation: key={Key}", key);
    return Results.NoContent();
}).RequireAuthorization();

app.MapControllers();
app.Run();
