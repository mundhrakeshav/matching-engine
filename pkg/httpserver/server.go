package httpserver

import (
	"context"
	"fmt"

	"github.com/labstack/echo/v5"
)

type Router interface {
	Use(middleware ...echo.MiddlewareFunc)
	Pre(middleware ...echo.MiddlewareFunc)
	Group(prefix string, middleware ...echo.MiddlewareFunc) (sg *echo.Group)
	GET(path string, h echo.HandlerFunc, m ...echo.MiddlewareFunc) echo.RouteInfo
	PATCH(path string, h echo.HandlerFunc, m ...echo.MiddlewareFunc) echo.RouteInfo
	POST(path string, h echo.HandlerFunc, m ...echo.MiddlewareFunc) echo.RouteInfo
	DELETE(path string, h echo.HandlerFunc, m ...echo.MiddlewareFunc) echo.RouteInfo
}

type Server struct {
	app     *echo.Echo
	notify  chan error
	address string
}

func NewServer(host string, port int) *Server {
	return &Server{
		app:     echo.New(),
		notify:  make(chan error),
		address: fmt.Sprintf("%s:%d", host, port),
	}
}

func (s *Server) Use(middlewares ...echo.MiddlewareFunc) {
	for i := range middlewares {
		s.app.Use(middlewares[i])
	}
}

func (s *Server) Pre(middlewares ...echo.MiddlewareFunc) {
	for i := range middlewares {
		s.app.Pre(middlewares[i])
	}
}

func (s *Server) Router() Router {
	return s.app
}

func (s *Server) Start(ctx context.Context) {
	go func() {
		sc := echo.StartConfig{
			Address:    s.address,
			HideBanner: true,
		}
		s.notify <- sc.Start(ctx, s.app)
	}()
}

func (s *Server) Notify() <-chan error {
	return s.notify
}
