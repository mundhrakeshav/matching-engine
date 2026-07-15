package v1

import (
	"net/http"

	"github.com/labstack/echo/v5"
)

type DataResponse struct {
	Data any `json:"data"`
}

type ErrorResponse struct {
	Message string `json:"message"`
}

func FormatSuccessResponse(c *echo.Context, data any) error {
	return c.JSON(http.StatusOK, DataResponse{Data: data})
}

func FormatErrorResponse(c *echo.Context, status int, message string) error {
	return c.JSON(status, ErrorResponse{Message: message})
}
