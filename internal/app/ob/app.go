package ob

import (
	"ob/config"
	"ob/pkg/log"
)

func Run(cfg *config.Config, logger log.Logger) error {
	logger.Info("starting application",
		log.String("host", cfg.Host),
		log.Int("port", cfg.Port),
		log.String("log_level", cfg.LogLevel),
	)
	return nil
}
