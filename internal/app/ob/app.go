package ob

import (
	"context"
	"errors"
	"net/http"
	"os"
	"os/signal"
	"syscall"

	"ob/config"
	"ob/internal/controller"
	"ob/internal/entity"
	"ob/internal/matching"
	"ob/pkg/httpserver"
	"ob/pkg/log"
)

func Run(cfg *config.Config, logger log.Logger) error {
	logger.Info("starting application",
		log.String("host", cfg.Server.Host),
		log.Int("port", cfg.Server.Port),
		log.String("log_level", cfg.Server.LogLevel),
	)

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	book := entity.NewBook()
	engine := matching.NewEngine(book)

	server := httpserver.NewServer(cfg.Server.Host, cfg.Server.Port)
	controller.SetupRouter(cfg.Server.ServiceName, server.Router(), engine)
	server.Start(ctx)

	quit := make(chan os.Signal, 1)
	signal.Notify(quit, os.Interrupt, syscall.SIGTERM)

	select {
	case err := <-server.Notify():
		if err != nil && !errors.Is(err, http.ErrServerClosed) {
			logger.Error("server stopped unexpectedly", log.Err(err))
			return err
		}
	case sig := <-quit:
		logger.Info("shutdown signal received", log.String("signal", sig.String()))
		cancel()
		if err := <-server.Notify(); err != nil && !errors.Is(err, http.ErrServerClosed) {
			logger.Error("server stopped with error", log.Err(err))
			return err
		}
	}

	logger.Info("application stopped")

	return nil
}
