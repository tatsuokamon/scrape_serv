#pragma once

#include <gumbo.h>
#include <string>

extern "C" void free_string(char* s);
bool has_id(GumboElement* el,  const char* id);
bool has_class(GumboElement* el,  const char* _class);
void inner_text(GumboNode* node,  std::string& out);

struct Tag {
	std::string name;
	std::string url;

	explicit Tag(std::string&& _name, std::string&& _url) noexcept : name(std::move(_name)), url(std::move(_url)){}

	// cannot copy
	Tag(const Tag&) = delete;
	Tag& operator=(const Tag&) = delete;

	Tag(Tag&& t) noexcept;
	Tag& operator=(Tag&& t) noexcept;
};
