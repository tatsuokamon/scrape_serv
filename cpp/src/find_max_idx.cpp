#include <cstdio>
#include <gumbo.h>
#include <gumbo_search_lib.hpp>
#include <regex>
#include <string>
#include <iostream>
// assumed structure
// <body>
	// <div id="container">
		// <div id="main">
			// <div> </div>
			// <div> </div>
			// ...
			// <div class="wp-pagenavi"> 
				// <div class="page">1 / 48</div>
			// </div>
		// </div>
	// </div>
// </body>
// 
// not found version
// <body>
	// <div id="container">
		// <div id="main">
			// <div> </div>
			// <div> </div>
			// ...
		// </div>
	// </div>
// </body>
// ---> container ---> main ---> wp-pagenavi ---> page
//                       |
//                       |
//                       |-----> not found wp-pagenavi
//                       
// so first let's find main div!

void find_main(GumboNode* node, GumboNode*& main) {
	if (!node || node->type != GUMBO_NODE_ELEMENT || main) {
		return;
	}
	GumboElement* el = &node->v.element;
	if (el->tag == GUMBO_TAG_DIV && has_id(el, "container")) {
		for (unsigned i = 0; i < el->children.length; i++) {
			GumboNode* child = static_cast<GumboNode*>(el->children.data[i]);
			if (child->type == GUMBO_NODE_ELEMENT) {
				GumboElement* inner = &child->v.element;
				if (inner->tag == GUMBO_TAG_DIV && has_id(inner , "main")) {
					main = child;
					return;
					}
				}
			}
		}

	for (unsigned i = 0; i < el->children.length; i++ ) {
		GumboNode* child = static_cast<GumboNode*>(el->children.data[i]);
		find_main(child, main);
	}
}

bool find_page(GumboNode* main, int& page) {
	if (!main) return false;
	GumboElement* el = &main->v.element;
	for (unsigned i = 0; i < el->children.length; i++ ) {
		GumboNode* child = static_cast<GumboNode*>(el->children.data[i]);
		if (child->type != GUMBO_NODE_ELEMENT) continue;
		GumboElement* inner = &child->v.element;
		if (inner->tag == GUMBO_TAG_DIV && has_class(inner, "wp-pagenavi")) {
			for (unsigned i = 0; i < inner->children.length; i++) {
				GumboNode* grand_child = (GumboNode*)inner->children.data[i];
				if (grand_child->type == GUMBO_NODE_ELEMENT && has_class(&grand_child->v.element, "pages")) {
					std::string text;
					inner_text(grand_child, text);
					std::cout << text << "\n";

					std::regex re(R"(\d+ / (\d+))");
					std::smatch m;

					if (!std::regex_search(text, m, re)) {return false;};
					std::cout << m[1];
					int idx = std::stoi(m[1]);
					page = idx;
					return true;
				}
			}
		}
	}

	return false;
}

extern "C" int find_max_idx(const char* html, int* page) {
	auto* output = gumbo_parse(html);
	*page = 0;
	if (!output) {
		return 1;
	}

	GumboNode* node = output->root;
	GumboNode* main = nullptr;
	int idx = 0;
	try {
		find_main(node, main);
		if (!main) {
			gumbo_destroy_output(&kGumboDefaultOptions, output);
			return 2;
		};
		if (!find_page(main, idx)) {
			gumbo_destroy_output(&kGumboDefaultOptions, output);
			return 3;
		}

		*page = idx;
		gumbo_destroy_output(&kGumboDefaultOptions, output);
		return 0;
	} catch (const std::exception& e) {
		std::cout << e.what() << std::endl;
		gumbo_destroy_output(&kGumboDefaultOptions, output);
		return 4;
	}

}
