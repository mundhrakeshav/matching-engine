package v1

import (
	"ob/internal/entity"

	"github.com/labstack/echo/v5"
)

type SubmitOrderRequest struct {
	Symbol    string               `json:"symbol"`
	Side      entity.OrderSideType `json:"side"`
	OrderKind entity.OrderKindType `json:"kind"`
}

func (h *Handler) MakeSubmitOrderHandler() echo.HandlerFunc {
	return func(c *echo.Context) error {
		return FormatSuccessResponse(c, map[string]string{
			"status": "ok",
		})
	}
}
