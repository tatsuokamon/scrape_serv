#include <cstdio>
#include <exception>
#include <iostream>
#include <nlohmann/detail/macro_scope.hpp>
#include <regex>
#include <cstdint>
#include <cstring>
#include <ctime>
#include <nlohmann/json_fwd.hpp>
#include <sstream>
#include <string>
#include "gumbo.h"
#include "date/date.h"
#include <nlohmann/json.hpp>
#include <gumbo_search_lib.hpp>
#include <strings.h>
#include <utility>

using json = nlohmann::json;
NLOHMANN_DEFINE_TYPE_NON_INTRUSIVE(Tag, url, name);

bool find_id_from_url(std::string url, std::string& id) {
	std::regex re(R"(rj[\d]+)");
	std::smatch m;

	if (!std::regex_search(url, m, re)) {return false;}; 
	id = m[0];
	return true;
}
void walk_finding_meta_post_chapter(
		GumboNode* node,
		std::string& title,
		std::string& url, 
		std::string& img, 
		int64_t& t,
		GumboNode* &post_tag,
		GumboNode* &chapter,
		bool& got_title,
		bool& got_url,
		bool& got_img,
		bool& got_time
		) {

	if (!node || node->type != GUMBO_NODE_ELEMENT) return;
	auto *el = &node->v.element;

	if (!got_url && el->tag == GUMBO_TAG_LINK) {
		GumboAttribute* rel = gumbo_get_attribute(&el->attributes, "rel");
		if (rel && strcmp(rel->value, "canonical") == 0) {
			auto* href = gumbo_get_attribute(&el->attributes, "href");
			if (href) {url = href->value; got_url = true;}
		}
	}

	if (!got_title && el->tag == GUMBO_TAG_TITLE) {
		inner_text(node, title);
		got_title = true;
	}

	if (!got_img && el->tag == GUMBO_TAG_VIDEO) {
		auto* p = gumbo_get_attribute(&el->attributes, "poster");
		if(p) {img = p->value; got_img = true;}
	}
 
	if (!got_time && el->tag == GUMBO_TAG_DIV && has_id(el, "post-time")) {
		std::string out;
		inner_text(node, out);
		std::stringstream iss(out);
		date::sys_seconds tp;
		iss >> date::parse("%Y年%m月%d日%H時", tp);
		t = tp.time_since_epoch().count();
		got_time = true;
	}

	if (el->tag == GUMBO_TAG_DIV && has_id(el, "post-tag")) post_tag = node;
	if (el->tag == GUMBO_TAG_DIV && has_id(el, "chapter")) chapter = node;

	if (got_title && got_img && got_url && got_time && post_tag && chapter) return;

	for (unsigned i = 0; i < el->children.length; ++i) {
		walk_finding_meta_post_chapter((GumboNode*)el->children.data[i], title, url, img, t, post_tag, chapter, got_title, got_url, got_img, got_time);
	}
}

struct TimeTable {
	int index;
	std::string title;
	std::string time;
	
	explicit TimeTable(int _index, std::string&& _title, std::string&& _time) noexcept : index(_index), title(std::move(_title)), time(std::move(_time)) {}

	// cannot copy
	TimeTable(const TimeTable&) = delete;
	TimeTable& operator=(const TimeTable&) = delete;

	TimeTable(TimeTable&& t) noexcept : index(t.index), title(std::move(t.title)), time(std::move(t.time)) {};
	TimeTable& operator=(TimeTable&& t) noexcept {
		index = t.index;
		title = std::move(t.title);
		time = std::move(t.time);

		return *this;
	};
};
NLOHMANN_DEFINE_TYPE_NON_INTRUSIVE(TimeTable, index, title, time);

void find_tags_from_post_div(GumboNode* node,  
		std::vector<Tag>& cv,
		std::vector<Tag>& circle,
		std::vector<Tag>& scenario,
		std::vector<Tag>& illust,
		std::vector<Tag>& genre,
		std::vector<Tag>& series
		) {
	std::string title;
	for (unsigned i = 0; i < node->v.element.children.length; i++) {
		GumboNode* child = static_cast<GumboNode*>(node->v.element.children.data[i]);

		if (child->type != GUMBO_NODE_ELEMENT) continue;
		GumboElement* el = &child->v.element;
		if (el->tag == GUMBO_TAG_SPAN) {
			std::string out;
			inner_text(child, out);
			title = out;
		} else if(!title.empty() && el->tag == GUMBO_TAG_A) {
			GumboAttribute* href_attr = gumbo_get_attribute(&el->attributes, "href");
			if (!href_attr)continue;

			std::string href = href_attr->value;
			std::string text;
			inner_text(child, text);

			Tag result(std::move(href), std::move(text));
			if (title == "声優") {
				cv.push_back(std::move(result));
			}
			if (title == "サークル") {
				circle.push_back(std::move(result));
			}
			if (title == "シナリオ") {
				scenario.push_back(std::move(result));
			}
			if (title == "イラスト") {
				illust.push_back(std::move(result));
			}
			if (title == "ジャンル") {
				genre.push_back(std::move(result));
			}
			if (title == "シリーズ") {
				series.push_back(std::move(result));
			}
		}
	}
}

std::vector<TimeTable> find_table_from_chapter_div(GumboNode* node) {
	std::vector<TimeTable> v{};
	int index  = 0;

	for (unsigned i = 0; i < node->v.element.children.length; i++) {
		GumboNode* child = static_cast<GumboNode*>(node->v.element.children.data[i]);
		if (child->type != GUMBO_NODE_ELEMENT || child->v.element.tag != GUMBO_TAG_A) continue;

		GumboElement* link = &child->v.element;
		std::string title, time;
		for (unsigned i = 0; i < link->children.length; i++) {
			GumboNode* inner = static_cast<GumboNode*>(link->children.data[i]);
			if (inner->type == GUMBO_NODE_TEXT) {
				title = inner->v.text.text;
				continue;
			}

			if (inner->type == GUMBO_NODE_ELEMENT && inner->v.element.tag == GUMBO_TAG_SPAN) {
				inner_text(inner, time);
				continue;
			}
		}

		if (!title.empty() && !time.empty()) {
			TimeTable result(index++, std::move(title), std::move(time));
			v.push_back(std::move(result));
		}
	}
	return v;
}

extern "C" int find_meta(const char* html, char** result_json) {
	auto* output = gumbo_parse(html);
	if (!output) {
		return 1;
	}
	GumboNode* node = output->root;
	std::string title;
	std::string url; 
	std::string img; 
	int64_t t = 0;
	GumboNode* post_tag = nullptr;
	GumboNode* chapter = nullptr;
	bool got_title = false;
	bool got_url = false;
	bool got_img = false;
	bool got_time = false;
	try {
		walk_finding_meta_post_chapter(
				node,
				title,
				url, 
				img, 
				t,
				post_tag,
				chapter,
				got_title,
				got_url,
				got_img,
				got_time
				); 
		if (url.empty() || title.empty() || t == 0) {
			gumbo_destroy_output(&kGumboDefaultOptions, output);
			return 2;
		}

		std::vector<Tag> cv;
		std::vector<Tag> circle;
		std::vector<Tag> scenario;
		std::vector<Tag> illust;
		std::vector<Tag> genre;
		std::vector<Tag> series;
		if (post_tag) {
			find_tags_from_post_div(post_tag, cv, circle, scenario, illust, genre, series);
		}
		std::vector<TimeTable> time_tables;
		if (chapter) {
			time_tables = find_table_from_chapter_div(chapter);
		}

		json j;
		std::string id;
		if (!find_id_from_url(url, id)) {
			gumbo_destroy_output(&kGumboDefaultOptions, output);
			return 3;
		}
		j["id"] = id;
		j["title"] = title;
		j["url"] = url;
		j["img_src"] = img.empty() ? nullptr : img;
		j["time"] = t;

		j["cv"] = cv;
		j["genre"] = genre;
		j["illust"] = illust;
		j["circle"] = circle;
		j["series"] = series;

		j["time_table"] = time_tables;
		std::string dumped = j.dump();
		*result_json = strdup(dumped.c_str());

		gumbo_destroy_output(&kGumboDefaultOptions, output);
	} catch (const std::exception& e) {
		std::cout << e.what() << std::endl;
		return 4;
	}
	return 0;
} 

