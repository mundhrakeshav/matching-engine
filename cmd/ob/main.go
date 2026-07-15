package main

import (
	stdlog "log"

	"ob/config"
	"ob/internal/app/ob"
	"ob/internal/pkg/log"
)

func main() {
	cfg, err := config.Load()
	if err != nil {
		stdlog.Fatal(err)
	}

	logger, err := log.NewLogger(cfg.ServiceName, cfg.LogLevel)
	if err != nil {
		stdlog.Fatal(err)
	}
	defer logger.Sync()

	if err := ob.Run(cfg, logger); err != nil {
		logger.Error("application failed", log.Err(err))
	}
}
