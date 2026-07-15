package v1

import (
	"github.com/labstack/echo/v5"

	"ob/internal/matching"
)

type Handler struct {
	engine *matching.Engine
}

func NewHandler(engine *matching.Engine) *Handler {
	return &Handler{
		engine: engine,
	}
}

func (h *Handler) MakeStatusHandler() echo.HandlerFunc {
	return func(c *echo.Context) error {
		return FormatSuccessResponse(c, map[string]string{
			"status": "ok",
		})
	}
}
