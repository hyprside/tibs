#pragma once
#include <aquamarine/backend/Backend.hpp>
#include <iostream>
#include <signal.h>
#define SP Hyprutils::Memory::CSharedPointer

#define RASSERT(expr, reason, ...)                                             \
	if (!(expr)) {                                                               \
		std::cout << std::format(                                                  \
				"\n==================================================================" \
				"========================\nASSERTION FAILED! \n\n{}\n\nat: line {} "   \
				"in {}\n",                                                               \
				std::format(reason, ##__VA_ARGS__), __LINE__,                          \
				([]() constexpr -> std::string {                                       \
					return std::string(__FILE__).substr(                                 \
							std::string(__FILE__).find_last_of('/') + 1);                    \
				})());                                                                 \
		std::cout << "[Rustamarine] Assertion failed!\n";                            \
		fflush(stdout);                                       \
		fflush(stderr);                                       \
		exit(SIGABRT);                                                            \
	}
#define panic(reason, ...)                                                     \
	{                                                                            \
		std::cout << std::format(                                                  \
				"\n==================================================================" \
				"========================\nPANIC! \n\n{}\n\nat: line {} in {}\n",        \
				std::format(reason, ##__VA_ARGS__), __LINE__,                          \
				([]() constexpr -> std::string {                                       \
					return std::string(__FILE__).substr(                                 \
							std::string(__FILE__).find_last_of('/') + 1);                    \
				})());                                                                 \
		std::cout << "[Rustamarine] Panic!\n";                                       \
		fflush(stdout);                                       \
		fflush(stderr);                                       \
		exit(SIGABRT);                                                            \
	}
static const char *aqLevelToString(Aquamarine::eBackendLogLevel level) {
	switch (level) {
	case Aquamarine::eBackendLogLevel::AQ_LOG_TRACE:
		return "TRACE";
	case Aquamarine::eBackendLogLevel::AQ_LOG_DEBUG:
		return "DEBUG";
	case Aquamarine::eBackendLogLevel::AQ_LOG_ERROR:
		return "ERROR";
	case Aquamarine::eBackendLogLevel::AQ_LOG_WARNING:
		return "WARNING";
	case Aquamarine::eBackendLogLevel::AQ_LOG_CRITICAL:
		return "CRITICAL";
	default:
		break;
	}

	return "UNKNOWN";
}
