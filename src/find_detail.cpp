#include <cstring>
#include <gumbo.h>
#include <nlohmann/json.hpp>
#include <string>
#include <vector>
#include <gumbo_search_lib.hpp>
#include <iostream>

// find main!
void walk_through(GumboNode* node, GumboNode*& main) {
	if (node->type != GUMBO_NODE_ELEMENT || main) {
		return;
	}

	GumboElement* el = &node->v.element;
	if (el->tag == GUMBO_TAG_DIV && has_id(el, "main")) {
		main = node;
		return;
	}
	for (unsigned i = 0; i < el->children.length; i++) {
		GumboNode* child = static_cast<GumboNode*>(el->children.data[i]);
		walk_through(child, main);
	}
}

// find urls from main! 
// assumed structure
// <div id="main">
	// <div class="pop..."><div/>
	// <div class="post-list">
		// <a href=> </a>
		// <a> </a>
		//...
	// <div/>
// <div/>
// 
std::vector<std::string>  find_detail_urls(GumboNode* node) {
	if (!node) {
		return {};
	}
	GumboElement* post_list = nullptr;
	// find post-list!
	for (unsigned i = 0; i < node->v.element.children.length; i++) {
		GumboNode* child = static_cast<GumboNode*>(node->v.element.children.data[i]);
		GumboElement* el = &child->v.element;
		if (el->tag == GUMBO_TAG_DIV && has_class(el, "post-list")) { 
			post_list = el;
		}
	}

	if (!post_list) {
		return {};
	}
	std::vector<std::string> v;
	for (unsigned i = 0; i < post_list->children.length; i++) {
		GumboNode* child = static_cast<GumboNode*>(post_list->children.data[i]);
		GumboElement* el = &child->v.element;

		if (el->tag == GUMBO_TAG_A) {
			GumboAttribute* attr = gumbo_get_attribute(&el->attributes, "href");
			if(attr){
				v.push_back(attr->value);
			}
		}
	}

	return v;
}

extern "C" int find_detail(const char* html, char** result_json) {
	auto* output = gumbo_parse(html);
	if (!output) {
		return 1;
	}
	try {
		GumboNode* node = output->root;
		GumboNode* main = nullptr;
		walk_through(node, main);

		if (!main) {
			gumbo_destroy_output(&kGumboDefaultOptions, output);
			return 2;
		}

		auto result = find_detail_urls(main);
		gumbo_destroy_output(&kGumboDefaultOptions, output);
		nlohmann::json j = result;
		*result_json = strdup(j.dump().c_str());

		return 0;

	} catch (const std::exception& e){
		std::cout << e.what() << "\n";
		gumbo_destroy_output(&kGumboDefaultOptions, output);
		return 3;
	}
}

