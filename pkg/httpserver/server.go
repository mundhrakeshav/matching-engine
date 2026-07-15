package httpserver

import (
	"log"

	"github.com/labstack/echo/v5"
)

type Server struct {
	app      *echo.Echo
	notifyCh chan error
}

func NewServer(serviceName, address string, logger log.Logger) *Server {
	app := echo.New()

	return &Server{
		app:      app,
		notifyCh: make(chan error),
	}
}
