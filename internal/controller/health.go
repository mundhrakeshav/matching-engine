package controller

import (
	"context"
	"net/http"

	"github.com/labstack/echo/v5"

	"ob/pkg/httpserver"
)

type HealthCheck struct {
	Name    string `json:"name"`
	Healthy bool   `json:"healthy"`
	Error   string `json:"error,omitempty"`
}

type CheckFunc func(ctx context.Context) HealthCheck

func registerHealthHandler(router httpserver.Router) {
	router.GET("/", func(c *echo.Context) error {
		return c.JSON(http.StatusOK, "Ok")
	})
}

func registerReadinessStatusHandler(router httpserver.Router, serviceName string) {
	router.GET("/status/"+serviceName, func(c *echo.Context) error {
		return c.JSON(http.StatusOK, map[string]any{
			"message": "service is up",
		})
	})
}

func registerReadinessProbeHandler(router httpserver.Router, healthChecks ...CheckFunc) {
	router.GET("/healthz", func(c *echo.Context) error {
		ctx := c.Request().Context()
		out := make([]HealthCheck, 0, len(healthChecks))

		for _, check := range healthChecks {
			out = append(out, check(ctx))
		}

		return c.JSON(http.StatusOK, map[string]any{
			"data": out,
		})
	})
}
