#include <gumbo.h>
#include <vector>
#include <gumbo_search_lib.hpp>
#include <nlohmann/json.hpp>
#include <iostream>
// find main!

NLOHMANN_DEFINE_TYPE_NON_INTRUSIVE(Tag, url, name);
void find_main_div(GumboNode* node, GumboNode*& main) {
	if(node->type != GUMBO_NODE_ELEMENT || main) {
		return;
	}

	GumboElement* el = &node->v.element;
	if (el->tag == GUMBO_TAG_DIV && has_id(el, "main")) {
		main = node;
		return;
	}

	for (unsigned i = 0; i < el->children.length; i++) {
		GumboNode* child = static_cast<GumboNode*>(el->children.data[i]);
		find_main_div(child, main);
	}
}

// find tag-list from main and then, extract tags
std::vector<Tag> extract_tags(GumboNode* main, int& resp) {
	GumboElement* el = &main->v.element;
	GumboNode* tag_list_node = nullptr;

	for (unsigned i = 0; i < el->children.length; i++) {
		GumboNode* child = static_cast<GumboNode*>(el->children.data[i]);
		GumboElement* child_el = &child->v.element;

		if (child_el->tag == GUMBO_TAG_DIV && has_class(child_el, "tag-list")) {
			tag_list_node = child;
		}
	}

	if (!tag_list_node) {
		resp = 100;
		return {};
	};
	GumboElement* tag_list_el = &tag_list_node->v.element;
	std::vector<Tag> result;
	for (unsigned i = 0; i < tag_list_el->children.length; i++) {
		GumboNode* child = static_cast<GumboNode*>(tag_list_el->children.data[i]);
		if (child->type != GUMBO_NODE_ELEMENT ) continue;
		GumboElement* child_el = &child->v.element;

		if (child_el->tag == GUMBO_TAG_A) {
			std::string text;
			for (unsigned j = 0; j < child_el->children.length; j++) {
				GumboNode* grand_child = (GumboNode*)child_el->children.data[j];
				if (grand_child->type != GUMBO_NODE_TEXT) {
					continue;
				}
				text = grand_child->v.text.text;
			}
			GumboAttribute* attr = gumbo_get_attribute(&child_el->attributes, "href");
			if (!attr) continue;
			std::string href = attr->value;

			result.push_back(Tag(std::move(text), std::move(href)));
		}
	}

	return result;
}

extern "C" int update_tag(const char* html, char** result_json) {
	auto* output = gumbo_parse(html);
	if (!output) {
		return 1;
	}

	try {
		GumboNode* node = output->root;
		GumboNode* main = nullptr;
		find_main_div(node, main);

		if (!main) {
			gumbo_destroy_output(&kGumboDefaultOptions, output);
			return 2;
		}

		int resp = 0;
		auto result = extract_tags(main, resp);
		gumbo_destroy_output(&kGumboDefaultOptions, output);
		nlohmann::json j = result;
		std::string dumped = j.dump();
		*result_json = strdup(dumped.c_str());

		return resp;
	} catch (const std::exception& e) {
		std::cout << e.what() << std::endl;
		return 4;
	}
}
