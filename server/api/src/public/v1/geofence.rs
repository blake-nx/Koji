use crate::utils::{request::send_api_req, response::Response};

use super::*;

use serde_json::json;

use model::{
    api::{
        args::{get_return_type, ApiQueryArgs, Args, ArgsUnwrapped, ReturnTypeArg},
        GeoFormats, ToCollection,
    },
    db::{area, geofence, instance, project},
    KojiDb, ScannerType,
};

#[get("/all")]
async fn all(
    conn: web::Data<KojiDb>,
    args: web::Query<ApiQueryArgs>,
) -> Result<HttpResponse, Error> {
    let args = args.into_inner();
    let fc = geofence::Query::get_all_collection(&conn.koji, &args)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    log::info!("[PUBLIC_API] Returning {} instances", fc.features.len());
    Ok(HttpResponse::Ok().json(Response {
        data: Some(json!(fc)),
        message: "Success".to_string(),
        status: "ok".to_string(),
        stats: None,
        status_code: 200,
    }))
}

#[get("/area/{geofence}")]
async fn get_area(
    conn: web::Data<KojiDb>,
    geofence: actix_web::web::Path<String>,
    args: web::Query<ApiQueryArgs>,
) -> Result<HttpResponse, Error> {
    let id = geofence.into_inner();
    let args = args.into_inner();
    let return_type = get_return_type(
        args.rt.clone().unwrap_or("feature".to_string()),
        &ReturnTypeArg::Feature,
    );

    let feature = geofence::Query::get_one_feature(&conn.koji, id, &args)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    log::info!(
        "[PUBLIC_API] Returning feature for {:?}",
        feature.property("name")
    );
    Ok(utils::response::send(
        feature.to_collection(None, None),
        return_type,
        None,
        false,
        None,
    ))
}

#[post("/save-koji")]
async fn save_koji(
    conn: web::Data<KojiDb>,
    payload: web::Json<Args>,
) -> Result<HttpResponse, Error> {
    let ArgsUnwrapped { area, .. } = payload.into_inner().init(Some("geofence_save"));

    geofence::Query::upsert_from_geometry(&conn.koji, GeoFormats::FeatureCollection(area))
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(Response {
        data: None,
        message: "Success".to_string(),
        status: "ok".to_string(),
        stats: None,
        status_code: 200,
    }))
}

#[post("/save-scanner")]
async fn save_scanner(
    conn: web::Data<KojiDb>,
    payload: web::Json<Args>,
) -> Result<HttpResponse, Error> {
    let ArgsUnwrapped { area, .. } = payload.into_inner().init(Some("geofence_save"));

    let (inserts, updates) = if conn.scanner_type == ScannerType::Unown {
        area::Query::upsert_from_geometry(&conn.controller, GeoFormats::FeatureCollection(area))
            .await
    } else {
        instance::Query::upsert_from_geometry(
            &conn.controller,
            GeoFormats::FeatureCollection(area),
            false,
        )
        .await
    }
    .map_err(actix_web::error::ErrorInternalServerError)?;

    let project = project::Query::get_scanner_project(&conn.koji)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    if let Some(project) = project {
        send_api_req(project, Some(&conn.scanner_type))
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;
    }
    log::info!("Rows Updated: {}, Rows Inserted: {}", updates, inserts);

    Ok(HttpResponse::Ok().json(Response {
        data: Some(json!({ "updates": updates, "inserts": inserts })),
        message: "Success".to_string(),
        status: "ok".to_string(),
        stats: None,
        status_code: 200,
    }))
}

#[get("/push/{id}")]
async fn push_to_prod(
    conn: web::Data<KojiDb>,
    id: actix_web::web::Path<String>,
) -> Result<HttpResponse, Error> {
    let id = id.into_inner();

    let feature = geofence::Query::get_one_feature(&conn.koji, id, &ApiQueryArgs::default())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let (inserts, updates) = if conn.scanner_type == ScannerType::Unown {
        area::Query::upsert_from_geometry(&conn.controller, GeoFormats::Feature(feature)).await
    } else {
        instance::Query::upsert_from_geometry(&conn.controller, GeoFormats::Feature(feature), false)
            .await
    }
    .map_err(actix_web::error::ErrorInternalServerError)?;

    let project = project::Query::get_scanner_project(&conn.koji)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    if let Some(project) = project {
        send_api_req(project, Some(&conn.scanner_type))
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;
    }
    log::info!("Rows Updated: {}, Rows Inserted: {}", updates, inserts);

    Ok(HttpResponse::Ok().json(Response {
        data: Some(json!({ "updates": updates, "inserts": inserts })),
        message: "Success".to_string(),
        status: "ok".to_string(),
        stats: None,
        status_code: 200,
    }))
}

#[get("/reference")]
async fn reference_data(conn: web::Data<KojiDb>) -> Result<HttpResponse, Error> {
    let fences = geofence::Query::get_all_no_fences(&conn.koji)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    log::info!("[GEOFENCES_ALL] Returning {} instances", fences.len());
    Ok(HttpResponse::Ok().json(Response {
        data: Some(json!(fences)),
        message: "Success".to_string(),
        status: "ok".to_string(),
        stats: None,
        status_code: 200,
    }))
}

#[get("/reference/{project}")]
async fn reference_data_project(
    conn: web::Data<KojiDb>,
    url: actix_web::web::Path<String>,
) -> Result<HttpResponse, Error> {
    let project = url.into_inner();
    let fences = geofence::Query::by_project(&conn.koji, project)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(Response {
        data: Some(json!(fences)),
        message: "Success".to_string(),
        status: "ok".to_string(),
        stats: None,
        status_code: 200,
    }))
}

#[get("/{return_type}")]
async fn specific_return_type(
    conn: web::Data<KojiDb>,
    url: actix_web::web::Path<String>,
    args: web::Query<ApiQueryArgs>,
) -> Result<HttpResponse, Error> {
    let return_type = url.into_inner();
    let args = args.into_inner();
    let return_type = get_return_type(return_type, &ReturnTypeArg::FeatureCollection);

    let fc = geofence::Query::get_all_collection(&conn.koji, &args)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    log::info!("[GEOFENCES_ALL] Returning {} instances", fc.features.len());
    Ok(utils::response::send(fc, return_type, None, false, None))
}

#[get("/{return_type}/{project}")]
async fn specific_project(
    conn: web::Data<KojiDb>,
    url: actix_web::web::Path<(String, String)>,
    args: web::Query<ApiQueryArgs>,
) -> Result<HttpResponse, Error> {
    let (return_type, project) = url.into_inner();
    let args = args.into_inner();

    let return_type = get_return_type(return_type, &ReturnTypeArg::FeatureCollection);
    let features = geofence::Query::project_as_feature(&conn.koji, project, &args)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    log::info!("[GEOFENCES_FC_ALL] Returning {} instances", features.len());
    Ok(utils::response::send(
        features.to_collection(None, None),
        return_type,
        None,
        false,
        None,
    ))
}
