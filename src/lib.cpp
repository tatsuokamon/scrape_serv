#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <gumbo.h>
#include <string>
#include <date/date.h>
#include <unistd.h>
#include <gumbo_search_lib.hpp>
#include <utility>
#include <nlohmann/json.hpp>

Tag::Tag(Tag&& t) noexcept : name(std::move(t.name)), url(std::move(t.url)) {}
Tag& Tag::operator=(Tag&& t) noexcept {
	url = std::move(t.url);
	name = std::move(t.name);

	return *this;
}

extern "C" void free_char(char* s) {
	if (s) {
		free(s);
	}
}

bool has_id(GumboElement* el,  const char* id) {
	GumboAttribute* attr = gumbo_get_attribute(&el->attributes, "id");
	return attr && strcmp(attr->value, id) == 0;
}

bool has_class(GumboElement* el,  const char* _class) {
	GumboAttribute* attr = gumbo_get_attribute(&el->attributes, "class");
	return attr && strcmp(attr->value, _class) == 0;
}

void inner_text(GumboNode* node,  std::string& out) {
	if (!node) return;

	switch (node->type) {
		case GUMBO_NODE_TEXT:
		case GUMBO_NODE_WHITESPACE:
			out.append(node->v.text.text);
			return;

		case GUMBO_NODE_ELEMENT: {
			auto* el = &node->v.element;
			if (el->tag == GUMBO_TAG_SCRIPT || el->tag == GUMBO_TAG_STYLE) return;
			auto* ch = &el->children;
			for (unsigned i = 0; i < ch->length; ++i) {
				inner_text((GumboNode*)ch->data[i], out);
			}
			return;
		 }
		default:
			 return;
	}
}
