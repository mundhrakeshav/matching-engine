package ob

import (
	"ob/config"
	"ob/pkg/log"
)

func Run(cfg *config.Config, logger log.Logger) error {
	logger.Info("starting application",
		log.String("host", cfg.Server.Host),
		log.Int("port", cfg.Server.Port),
		log.String("log_level", cfg.Server.LogLevel),
	)
	return nil
}
