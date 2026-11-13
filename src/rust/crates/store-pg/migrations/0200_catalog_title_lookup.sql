-- v1-manual: before a person enters an item by hand, the items of the same kind with the same
-- title (case and surrounding blanks folded) are listed ("Already here"). The lookup compares
-- exactly this expression (`CatalogRepo::titled`).
CREATE INDEX catalog_items_kind_title ON catalog_items (kind, lower(btrim(title)));
