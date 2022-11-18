-- if user_id is null, find all reservations within during for the resource
-- if resource_id is null, find all reservations within during for the user
-- if both are null, find all reservations within during
-- if both set, find all reservations within during for the resource and user

CREATE OR REPLACE FUNCTION rsvp.query(
    uid text, 
    rid text, 
    during TSTZRANGE,
    status rsvp.reservation_status,
    page integer DEFAULT 1,
    is_desc BOOL DEFAULT FALSE,
    page_size INTEGER DEFAULT 10
) RETURNS TABLE (LIKE rsvp.reservations) 
AS $$ 
DECLARE
    _sql TEXT;
BEGIN
    -- if page_size is not between 10 and 100, set it to 10
    IF page_size < 10 OR page_size > 100 THEN
        page_size := 10;
    END IF;

    -- if page is less than 1, set it to 1
    IF page < 1 THEN
        page := 1;
    END IF;

    -- format the query based on parameters
    _sql := format(
        'SELECT * FROM rsvp.reservations WHERE %L @> timespan AND status = %L AND %s ORDER BY LOWER(timespan) %s LIMIT %L::integer OFFSET %L::integer', 
        during,
        status,
        CASE
            WHEN uid IS NULL AND rid IS NULL THEN 'TRUE'
            WHEN uid IS NULL THEN 'resource_id = ' || QUOTE_LITERAL(rid)
            WHEN rid IS NULL THEN 'user_id = ' || QUOTE_LITERAL(uid)
            ELSE 'user_id = ' || QUOTE_LITERAL(uid) || ' AND resource_id = ' || QUOTE_LITERAL(rid)
        END,
        CASE
            WHEN is_desc THEN 'DESC'
            ELSE 'ASC'
        END,
        page_size,
        (page - 1) * page_size
    );

    -- log the sql
    RAISE NOTICE '%', _sql;

    -- execute the query
    RETURN QUERY EXECUTE _sql;
    -- Normally, you have to create new migrations to modify the database, 
    -- especially if you have deployed an online environment, 
    -- as you are now in the development learning stage, do so for comparison

    -- -- if both are null, find all reservations within during
    -- IF uid IS NULL AND rid IS NULL THEN
    --     RETURN QUERY SELECT * FROM rsvp.reservations WHERE timespan @> during;
    -- ELSEIF uid IS NULL THEN
    --     -- if user_id is null, find all reservations within during for the resource
    --     RETURN QUERY SELECT * FROM rsvp.reservations WHERE resource_id = rid AND during @> timespan;
    -- ELSEIF rid IS NULL THEN
    --     -- if resource_id is null, find all reservations within during for the user
    --     RETURN QUERY SELECT * FROM rsvp.reservations WHERE user_id = uid AND during @> timespan;
    -- ELSE
    --     -- if both set, find all reservations within during for the resource and user
    --     RETURN QUERY SELECT * FROM rsvp.reservations WHERE resource_id = rid AND user_id = uid AND during @> timespan;
    -- END IF;
END;
$$ LANGUAGE plpgsql;

-- we filter 2 more items one for starting, one for ending
-- If starting existing, then we have previous page,
-- If ending existing, then we have next page
CREATE OR REPLACE FUNCTION rsvp.filter(
    uid text, 
    rid text, 
    status rsvp.reservation_status,
    cursor bigint DEFAULT NULL,
    is_desc BOOL DEFAULT FALSE,
    page_size INTEGER DEFAULT 10
) RETURNS TABLE (LIKE rsvp.reservations) 
AS $$ 
DECLARE
    _sql TEXT;
    _offset bigint;
BEGIN
    -- if page_size is not between 10 and 100, set it to 10
    IF page_size < 10 OR page_size > 100 THEN
        page_size := 10;
    END IF;

    -- if cursor is NULL or less than 0, set it to 0 if is_desc is false, or to 2^63 - 1 if is_desc is true 
    IF cursor IS NULL OR cursor < 0 THEN
        IF is_desc THEN
            cursor := 9223372036854775807;
        ELSE
            cursor := 0;
        END IF;
    END IF;

    -- format the query based on parameters
    _sql := format(
        'SELECT * FROM rsvp.reservations WHERE %s AND status = %L AND %s ORDER BY id %s LIMIT %L::integer', 
        CASE 
            WHEN is_desc THEN 'id <= ' || cursor
            ELSE 'id >= ' || cursor
        END,
        status,
        CASE
            WHEN uid IS NULL AND rid IS NULL THEN 'TRUE'
            WHEN uid IS NULL THEN 'resource_id = ' || QUOTE_LITERAL(rid)
            WHEN rid IS NULL THEN 'user_id = ' || QUOTE_LITERAL(uid)
            ELSE 'user_id = ' || QUOTE_LITERAL(uid) || ' AND resource_id = ' || QUOTE_LITERAL(rid)
        END,
        CASE
            WHEN is_desc THEN 'DESC'
            ELSE 'ASC'
        END,
        page_size + 1
    );

    -- log the sql
    RAISE NOTICE '%', _sql;

    -- execute the query
    RETURN QUERY EXECUTE _sql;
END;
$$ LANGUAGE plpgsql;
