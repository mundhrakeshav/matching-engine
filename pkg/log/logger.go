package log

import (
	"context"
	"fmt"

	"go.uber.org/zap"
	"go.uber.org/zap/zapcore"
)

const (
	fieldService   = "service"
	fieldComponent = "component"
)

// Field is a structured log field.
type Field = zapcore.Field

func String(key, value string) Field {
	return zap.String(key, value)
}

func Int(key string, value int) Field {
	return zap.Int(key, value)
}

func Any(key string, value any) Field {
	return zap.Any(key, value)
}

func Err(err error) Field {
	return zap.Error(err)
}

type Logger interface {
	Info(msg string, fields ...Field)
	Warn(msg string, fields ...Field)
	Error(msg string, fields ...Field)
	Debug(msg string, fields ...Field)
	Component(name string) Logger
	With(fields ...Field) Logger
	WithContext(ctx context.Context) Logger
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

	logger := &zapLogger{logger: zl}
	defaultLogger = logger

	return logger, nil
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

func (l *zapLogger) WithContext(ctx context.Context) Logger {
	if logger, ok := ctx.Value(loggerCtxKey{}).(Logger); ok {
		return logger
	}

	var fields []Field
	if field, ok := requestIDFieldFromContext(ctx); ok {
		fields = append(fields, field)
	}
	if field, ok := trackingIDFieldFromContext(ctx); ok {
		fields = append(fields, field)
	}
	if len(fields) == 0 {
		return l
	}

	return l.With(fields...)
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
