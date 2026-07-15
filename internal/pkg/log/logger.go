package log

import (
	"fmt"

	"go.uber.org/zap"
)

const (
	fieldService   = "service"
	fieldComponent = "component"
)

type Logger interface {
	Info(msg string, fields ...Field)
	Warn(msg string, fields ...Field)
	Error(msg string, fields ...Field)
	Debug(msg string, fields ...Field)
	Component(name string) Logger
	With(fields ...Field) Logger
	Sync() error
}

type zapLogger struct {
	logger *zap.Logger
}

func NewLogger(serviceName, level string) (Logger, error) {
	lvl, err := parseLevel(level)
	if err != nil {
		return nil, err
	}

	cfg := zap.NewProductionConfig()
	cfg.Level = lvl
	cfg.OutputPaths = []string{"stdout"}
	cfg.ErrorOutputPaths = []string{"stderr"}

	zl, err := cfg.Build(zap.Fields(zap.String(fieldService, serviceName)))
	if err != nil {
		return nil, fmt.Errorf("build zap logger: %w", err)
	}

	return &zapLogger{logger: zl}, nil
}

func (l *zapLogger) Info(msg string, fields ...Field) {
	l.logger.Info(msg, fields...)
}

func (l *zapLogger) Warn(msg string, fields ...Field) {
	l.logger.Warn(msg, fields...)
}

func (l *zapLogger) Error(msg string, fields ...Field) {
	l.logger.Error(msg, fields...)
}

func (l *zapLogger) Debug(msg string, fields ...Field) {
	l.logger.Debug(msg, fields...)
}

func (l *zapLogger) Component(name string) Logger {
	return &zapLogger{
		logger: l.logger.With(zap.String(fieldComponent, name)),
	}
}

func (l *zapLogger) With(fields ...Field) Logger {
	return &zapLogger{
		logger: l.logger.With(fields...),
	}
}

func (l *zapLogger) Sync() error {
	return l.logger.Sync()
}

func parseLevel(level string) (zap.AtomicLevel, error) {
	var lvl zap.AtomicLevel
	if err := lvl.UnmarshalText([]byte(level)); err != nil {
		return lvl, fmt.Errorf("parse log level %q: %w", level, err)
	}
	return lvl, nil
}
